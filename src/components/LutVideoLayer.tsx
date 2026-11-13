import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { editorApi } from "../services/editorApi";
import type { ChromaKey, ClipLut } from "../types/project";
import type { ColorMatrix } from "./colorAdjustments";
import {
  decodeLutTable,
  LUT_FRAGMENT_SHADER,
  LUT_VERTEX_SHADER,
  packColorMatrix,
  parseKeyColor,
  type DecodedLutTable,
} from "./lutPreview";

export type LutPreviewState =
  | { status: "loading" }
  | { status: "ready"; table: DecodedLutTable }
  | { status: "error"; message: string };

const tableCache = new Map<string, Promise<DecodedLutTable>>();

function loadLutTable(lut: ClipLut): Promise<DecodedLutTable> {
  const key = `${lut.path}\u0000${lut.sha256}`;
  let pending = tableCache.get(key);
  if (!pending) {
    pending = editorApi.readLutTable(lut.path, lut.sha256).then(decodeLutTable);
    // A failed load must be retried after the user fixes the file, so it is never cached.
    pending.catch(() => tableCache.delete(key));
    tableCache.set(key, pending);
  }
  return pending;
}

/** Loads the LUT table for a clip. Errors are surfaced, never replaced by an unprocessed frame. */
export function useLutTable(lut: ClipLut | null | undefined): LutPreviewState | null {
  const [state, setState] = useState<LutPreviewState | null>(null);
  const path = lut?.path;
  const sha256 = lut?.sha256;
  useEffect(() => {
    if (!path || !sha256) {
      setState(null);
      return undefined;
    }
    let cancelled = false;
    setState({ status: "loading" });
    loadLutTable({ name: "", path, sha256 }).then(
      (table) => {
        if (!cancelled) setState({ status: "ready", table });
      },
      (error: unknown) => {
        if (!cancelled) {
          setState({ status: "error", message: error instanceof Error ? error.message : String(error) });
        }
      }
    );
    return () => {
      cancelled = true;
    };
  }, [path, sha256]);
  return state;
}

interface LutVideoLayerProps {
  video: HTMLVideoElement | null;
  /** Null when the clip only has a chroma key. */
  table: DecodedLutTable | null;
  chroma: ChromaKey | null;
  matrix: ColorMatrix;
  style: CSSProperties;
  onFailure: (message: string) => void;
}

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error("Could not create a shader");
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    throw new Error(gl.getShaderInfoLog(shader) ?? "Shader compilation failed");
  }
  return shader;
}

/**
 * Draws the video's current frame through color matrix -> 3D LUT (same order as export).
 * The `<video>` keeps playing and seeking underneath; this canvas only replaces what is shown.
 */
export function LutVideoLayer({ video, table, chroma, matrix, style, onFailure }: LutVideoLayerProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const matrixRef = useRef(matrix);
  matrixRef.current = matrix;
  const chromaRef = useRef(chroma);
  chromaRef.current = chroma;
  const matrixKeyRef = useRef("");
  matrixKeyRef.current = `${matrix.map((row) => row.join(",")).join(";")}|${JSON.stringify(chroma)}`;
  const failureRef = useRef(onFailure);
  failureRef.current = onFailure;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !video) return undefined;
    const gl = canvas.getContext("webgl2", { alpha: true, premultipliedAlpha: false, antialias: false });
    if (!gl) {
      failureRef.current("WebGL2 is not available");
      return undefined;
    }

    let frame = 0;
    let lastKey = "";
    let lost = false;
    let program: WebGLProgram | null = null;
    let videoTexture: WebGLTexture | null = null;
    let lutTexture: WebGLTexture | null = null;

    const onLost = (event: Event) => {
      event.preventDefault();
      lost = true;
      failureRef.current("The graphics context was lost");
    };
    canvas.addEventListener("webglcontextlost", onLost);

    try {
      program = gl.createProgram();
      if (!program) throw new Error("Could not create a shader program");
      gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, LUT_VERTEX_SHADER));
      gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, LUT_FRAGMENT_SHADER));
      gl.linkProgram(program);
      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
        throw new Error(gl.getProgramInfoLog(program) ?? "Shader link failed");
      }
      gl.useProgram(program);

      videoTexture = gl.createTexture();
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, videoTexture);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

      lutTexture = gl.createTexture();
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_3D, lutTexture);
      gl.texParameteri(gl.TEXTURE_3D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_3D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_3D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_3D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_3D, gl.TEXTURE_WRAP_R, gl.CLAMP_TO_EDGE);
      // Without a LUT a neutral 2x2x2 texture keeps the sampler complete; u_has_lut is 0.
      const n = table ? table.size : 2;
      const rgba = new Float32Array(n * n * n * 4);
      for (let i = 0; i < n * n * n; i += 1) {
        if (table) {
          rgba[i * 4] = table.samples[i * 3];
          rgba[i * 4 + 1] = table.samples[i * 3 + 1];
          rgba[i * 4 + 2] = table.samples[i * 3 + 2];
        } else {
          rgba[i * 4] = (i % 2) as number;
          rgba[i * 4 + 1] = (Math.floor(i / 2) % 2) as number;
          rgba[i * 4 + 2] = Math.floor(i / 4) as number;
        }
        rgba[i * 4 + 3] = 1;
      }
      // RGBA16F is filterable in core WebGL2; RGBA32F would need an optional extension.
      gl.texImage3D(gl.TEXTURE_3D, 0, gl.RGBA16F, n, n, n, 0, gl.RGBA, gl.FLOAT, rgba);

      gl.uniform1i(gl.getUniformLocation(program, "u_video"), 0);
      gl.uniform1i(gl.getUniformLocation(program, "u_lut"), 1);
      gl.uniform1i(gl.getUniformLocation(program, "u_has_lut"), table ? 1 : 0);
      gl.uniform3fv(gl.getUniformLocation(program, "u_domain_min"), table ? table.domainMin : [0, 0, 0]);
      gl.uniform3fv(gl.getUniformLocation(program, "u_domain_max"), table ? table.domainMax : [1, 1, 1]);
      gl.uniform1f(gl.getUniformLocation(program, "u_lut_size"), n);
    } catch (error) {
      failureRef.current(error instanceof Error ? error.message : String(error));
      canvas.removeEventListener("webglcontextlost", onLost);
      return undefined;
    }

    const matrixLocation = gl.getUniformLocation(program, "u_matrix");
    const offsetLocation = gl.getUniformLocation(program, "u_offset");
    const keyEnabled = gl.getUniformLocation(program, "u_key_enabled");
    const keyColor = gl.getUniformLocation(program, "u_key_color");
    const keySimilarity = gl.getUniformLocation(program, "u_key_similarity");
    const keyBlend = gl.getUniformLocation(program, "u_key_blend");
    const despillMode = gl.getUniformLocation(program, "u_despill_mode");
    const despillMix = gl.getUniformLocation(program, "u_despill_mix");

    const draw = () => {
      frame = requestAnimationFrame(draw);
      if (lost || video.readyState < 2 || video.videoWidth === 0) return;
      const key = `${video.currentTime}|${video.videoWidth}x${video.videoHeight}|${matrixKeyRef.current}`;
      if (video.paused && key === lastKey) return;
      lastKey = key;
      if (canvas.width !== video.videoWidth || canvas.height !== video.videoHeight) {
        canvas.width = video.videoWidth;
        canvas.height = video.videoHeight;
      }
      gl.viewport(0, 0, canvas.width, canvas.height);
      const packed = packColorMatrix(matrixRef.current);
      gl.uniformMatrix3fv(matrixLocation, false, packed.mat3);
      gl.uniform3fv(offsetLocation, packed.offset);
      const keyed = chromaRef.current;
      gl.uniform1i(keyEnabled, keyed ? 1 : 0);
      if (keyed) {
        const [kr, kg, kb] = parseKeyColor(keyed.key_color);
        gl.uniform3f(keyColor, kr / 255, kg / 255, kb / 255);
        gl.uniform1f(keySimilarity, keyed.similarity);
        gl.uniform1f(keyBlend, keyed.blend);
        gl.uniform1i(despillMode, keyed.despill ? (keyed.despill.screen === "Blue" ? 2 : 1) : 0);
        gl.uniform1f(despillMix, keyed.despill?.mix ?? 0.5);
      }
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, videoTexture);
      try {
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, video);
      } catch (error) {
        // A tainted or unreadable frame must be reported, not drawn unprocessed.
        lost = true;
        failureRef.current(error instanceof Error ? error.message : String(error));
        return;
      }
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    };
    frame = requestAnimationFrame(draw);

    return () => {
      cancelAnimationFrame(frame);
      canvas.removeEventListener("webglcontextlost", onLost);
      gl.deleteTexture(videoTexture);
      gl.deleteTexture(lutTexture);
      if (program) gl.deleteProgram(program);
    };
    // Matrix changes are picked up per frame through matrixRef / matrixKeyRef.
  }, [video, table]);

  return <canvas ref={canvasRef} data-testid="lut-video-layer" aria-hidden="true" style={style} />;
}
