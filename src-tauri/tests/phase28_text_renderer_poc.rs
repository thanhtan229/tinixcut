use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;
use tinixcut_lib::modules::media::text_renderer_poc::{
    compute_roi_mae, compute_roi_ssim, default_open_source_font_bytes, invoke_poc_preview_ipc,
    transform_alpha_mask, BackgroundBoxDef, ColorRgba, RgbaBuffer, ShapeLayerDef,
    SharedTextRendererPoc, DEFAULT_CACHE_MAX_BYTES,
};

const EXPECTED_ROBOTO_BOLD_SHA256: &str =
    "61F89F8DB49261C2F6106E8DCCC35DF7B2F7ED909020DB40A3FC905E95F99334";

fn get_open_source_fixture_font() -> Vec<u8> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let font_path = manifest_dir.join("fixtures/fonts/Roboto-Bold.ttf");
    if font_path.exists() {
        fs::read(font_path).expect("Read open-source Roboto-Bold fixture font")
    } else {
        default_open_source_font_bytes()
    }
}

fn get_root_artifact_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().expect("Root workspace dir");
    root.join("artifacts/animated-text-templates/phase-28")
}

fn get_ffmpeg_bin() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("resources/bin/ffmpeg.exe")
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 * p).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    sorted[idx]
}

#[test]
fn test_p28_ac01_shaping_and_cluster_mapping_with_rotation() {
    let font_bytes = get_open_source_fixture_font();
    let renderer = SharedTextRendererPoc::new(1920, 1080, font_bytes);
    let sample_text = "Tinix — Thử nghiệm tiếng Việt";

    // 1. Verify Font Asset SHA-256 Identity & Zero Missing Glyphs (.notdef)
    assert_eq!(
        renderer.font_sha256(),
        EXPECTED_ROBOTO_BOLD_SHA256,
        "Font asset SHA-256 must match pinned Apache-2.0 Roboto-Bold.ttf"
    );
    assert_eq!(
        renderer.count_missing_glyphs(sample_text),
        0,
        "All Vietnamese characters in sample text must resolve to valid non-zero GlyphIds"
    );

    // 2. NFC / NFD Equivalence Test: NFD string must produce identical layout, widths, and unified cluster masks
    let sample_nfd =
        "Tinix — Thu\u{031b}\u{0309} nghie\u{0323}\u{0302}m tie\u{0302}\u{0301}ng Vie\u{0323}\u{0302}t";
    let chars_nfc = renderer.evaluate_sample_motion(sample_text, 0.5);
    let chars_nfd = renderer.evaluate_sample_motion(sample_nfd, 0.5);

    assert_eq!(chars_nfc.len(), 29, "Exact 29 extended grapheme clusters");
    assert_eq!(
        chars_nfd.len(),
        29,
        "NFD string must normalize to exact 29 graphemes"
    );

    let non_space_count = chars_nfc.iter().filter(|c| c.grapheme != " ").count();
    assert_eq!(
        non_space_count, 24,
        "Exact 24 non-space characters (29 graphemes minus 5 spaces)"
    );

    for (a, b) in chars_nfc.iter().zip(chars_nfd.iter()) {
        assert_eq!(a.grapheme, b.grapheme);
        assert!((a.base_x - b.base_x).abs() < 1e-4);
        assert!((a.width - b.width).abs() < 1e-4);
    }

    // Diacritic tone marks must stay combined with base vowels
    assert_eq!(
        chars_nfc[10].grapheme, "ử",
        "Diacritic 'ử' must stay combined"
    );
    assert_eq!(
        chars_nfc[16].grapheme, "ệ",
        "Diacritic 'ệ' must stay combined"
    );
    assert_eq!(
        chars_nfc[27].grapheme, "ệ",
        "Diacritic 'ệ' in 'Việt' must stay combined"
    );

    // 3. Verify Group A (first 21 non-space chars) vs Group B (last 3 non-space chars: 'i', 'ệ', 't' at indices 26, 27, 28)
    // At t=0.80, valid_idx 23 ('ệ' at index 27) is actively rotating between start_f=111 and end_f1=131
    let chars_t08 = renderer.evaluate_sample_motion(sample_text, 0.80);
    assert_eq!(
        chars_t08[0].rotation_z_deg, 0.0,
        "Group A character must have 0 Z-rotation"
    );
    let rot_char = &chars_t08[27];
    assert!(
        rot_char.rotation_z_deg < -180.0 && rot_char.rotation_z_deg >= -360.0,
        "Group B bounce character 'ệ' at t=0.80 must have active Z-rotation, got {}",
        rot_char.rotation_z_deg
    );

    // 4. Verify unified cluster rasterization (1 unified mask per cluster)
    let masks = renderer.rasterize_cluster_glyphs(rot_char);
    assert_eq!(
        masks.len(),
        1,
        "Cluster must rasterize into a single unified alpha mask before rotation"
    );
    assert!(masks[0].width > 0 && masks[0].height > 0);
}

#[test]
fn test_p28_ac02_deterministic_overlay_generation() {
    let font_bytes = get_open_source_fixture_font();
    let renderer = SharedTextRendererPoc::new(1920, 1080, font_bytes);
    let text = "Tinix — Thử nghiệm tiếng Việt";

    renderer.clear_cache();
    let buf_cold = renderer.render_frame(text, 0.5);
    let buf_warm = renderer.render_frame(text, 0.5);

    assert_eq!(buf_cold.width, 1920);
    assert_eq!(buf_cold.height, 1080);
    assert_eq!(buf_cold.data.len(), buf_warm.data.len());
    assert_eq!(
        buf_cold.data, buf_warm.data,
        "Cold cache and Warm cache overlay outputs must be 100% bit-for-bit identical"
    );
}

#[test]
fn test_p28_ac03_png_export_preview_ipc_and_parity_ssim() {
    let font_bytes = get_open_source_fixture_font();
    let renderer = SharedTextRendererPoc::new(1920, 1080, font_bytes);
    let text = "Tinix — Thử nghiệm tiếng Việt";

    // Consumer 1: Tauri Preview IPC packet + helper invocation
    let preview_packet = renderer.render_preview_packet(text, 0.5);
    let ipc_response = invoke_poc_preview_ipc(text, 0.5);

    assert_eq!(preview_packet.width, 1920);
    assert_eq!(preview_packet.height, 1080);
    assert!(preview_packet.has_visible_content);
    assert_eq!(ipc_response.roi, preview_packet.roi);
    assert_eq!(ipc_response.font_sha256, EXPECTED_ROBOTO_BOLD_SHA256);
    assert!(
        ipc_response.roi_png_bytes.len() < preview_packet.png_bytes.len() / 10,
        "Tight ROI PNG must reduce IPC transport payload by >10x"
    );

    // Consumer 2: Direct lossless frame buffer for export pipeline
    let export_buf = renderer.render_frame(text, 0.5);
    let export_roi = export_buf.compute_dirty_roi().expect("Valid export ROI");

    // Layout metrics tolerance check (<= 1.0 px)
    for i in 0..4 {
        let diff = (preview_packet.roi[i] as i32 - export_roi[i] as i32).abs();
        assert!(
            diff <= 1,
            "Preview vs Export ROI layout bounds must match within <= 1px"
        );
    }

    // Round-trip PNG decode & Text ROI SSIM check (>= 0.985)
    let decoded_preview =
        RgbaBuffer::from_png_bytes(&preview_packet.png_bytes).expect("Decode preview PNG");
    let roi_ssim = compute_roi_ssim(&decoded_preview, &export_buf, preview_packet.roi);
    let roi_mae = compute_roi_mae(&decoded_preview, &export_buf, preview_packet.roi);

    println!(
        "P28-AC-03 Parity: Dirty ROI = {:?}, ROI PNG = {} bytes, ROI SSIM = {:.6}, ROI MAE = {:.6}",
        preview_packet.roi,
        preview_packet.roi_png_bytes.len(),
        roi_ssim,
        roi_mae
    );

    assert!(
        roi_ssim >= 0.985,
        "Text ROI SSIM between Preview and Export must be >= 0.985, got {:.6}",
        roi_ssim
    );
    assert_eq!(roi_mae, 0.0, "Lossless overlay MAE must be 0.0");

    let out_dir = get_root_artifact_dir();
    fs::create_dir_all(&out_dir).expect("Create phase-28 artifacts dir");
    let out_path = out_dir.join("poc_overlay_t05.png");
    fs::write(&out_path, &preview_packet.png_bytes).expect("Write PNG artifact");
    let roi_path = out_dir.join("poc_overlay_roi_t05.png");
    fs::write(&roi_path, &preview_packet.roi_png_bytes).expect("Write ROI PNG artifact");
    assert!(out_path.exists() && roi_path.exists());

    // 4. Measure Tauri Preview IPC end-to-end delivery latency (including JSON serialization)
    let t_ipc_start = Instant::now();
    let ipc_resp = invoke_poc_preview_ipc(text, 0.5);
    let ipc_json = serde_json::to_vec(&ipc_resp).expect("Serialize IPC response to JSON");
    let ipc_delivery_ms = t_ipc_start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "P28-AC-03 Preview IPC Delivery: {} bytes JSON, delivery latency = {:.2} ms (render_elapsed = {:.2} ms)",
        ipc_json.len(),
        ipc_delivery_ms,
        ipc_resp.render_elapsed_ms
    );
    assert!(
        ipc_delivery_ms <= 16.0,
        "Preview IPC delivery latency must stay within 16.0 ms budget, got {:.2} ms",
        ipc_delivery_ms
    );
    assert!(ipc_resp.has_visible_content);
    assert!(!ipc_resp.roi_png_bytes.is_empty());
}

#[test]
fn test_p28_ac04_alpha_channel_integrity_four_backgrounds() {
    let font_bytes = get_open_source_fixture_font();
    let renderer = SharedTextRendererPoc::new(1920, 1080, font_bytes);
    let text = "Tinix — Thử nghiệm tiếng Việt";

    // Render at t=1.0 (full completion) and t=0.80 (active bounce & Z-rotation)
    let overlay = renderer.render_frame(text, 1.0);
    let overlay_rot = renderer.render_frame(text, 0.80);
    let roi = overlay.compute_dirty_roi().expect("Non-empty overlay ROI");
    let roi_rot = overlay_rot
        .compute_dirty_roi()
        .expect("Non-empty rotated overlay ROI");

    // 1. Verify no edge clipping on either steady or rotated/bouncing frames
    assert!(
        roi[0] >= 20 && roi[1] >= 20 && roi[2] <= 1900 && roi[3] <= 1060,
        "Steady overlay must not clip canvas boundaries: {:?}",
        roi
    );
    assert!(
        roi_rot[0] >= 20 && roi_rot[1] >= 20 && roi_rot[2] <= 1900 && roi_rot[3] <= 1060,
        "Rotated/bouncing overlay must not clip canvas boundaries: {:?}",
        roi_rot
    );

    // 1b. Non-clipping proof at ROI perimeter: ensure pixels at the outermost boundary of dirty_roi have zero alpha
    // (i.e. neither stroke nor blurred shadow is abruptly sliced by a clipped bounding box)
    let check_perimeter_zero = |buf: &RgbaBuffer, r: [u32; 4]| {
        // Expand ROI by 1 pixel to test that the background immediately adjacent to the bounding box is completely transparent
        let x0 = r[0].saturating_sub(1);
        let y0 = r[1].saturating_sub(1);
        let x1 = (r[2] + 1).min(buf.width);
        let y1 = (r[3] + 1).min(buf.height);

        // Top and bottom boundary row checks
        for x in x0..x1 {
            assert_eq!(
                buf.get_pixel(x, y0)[3],
                0,
                "Top boundary must be transparent at x={}",
                x
            );
            assert_eq!(
                buf.get_pixel(x, y1 - 1)[3],
                0,
                "Bottom boundary must be transparent at x={}",
                x
            );
        }
        // Left and right boundary column checks
        for y in y0..y1 {
            assert_eq!(
                buf.get_pixel(x0, y)[3],
                0,
                "Left boundary must be transparent at y={}",
                y
            );
            assert_eq!(
                buf.get_pixel(x1 - 1, y)[3],
                0,
                "Right boundary must be transparent at y={}",
                y
            );
        }
    };
    check_perimeter_zero(&overlay, roi);
    check_perimeter_zero(&overlay_rot, roi_rot);

    // 1c. Verify geometry & bounds preservation across scale factors (0.59x, 1.0x, 1.208x)
    let char_layout_base = &renderer.evaluate_sample_motion(text, 1.0)[0];
    let mask_base = renderer.rasterize_cluster_glyphs(char_layout_base);
    assert!(!mask_base.is_empty(), "Must rasterize base glyph mask");
    let w_base = mask_base[0].width as f32;
    let _h_base = mask_base[0].height as f32;

    // Test scale 0.59x (CapCut combined scale factor)
    let mask_scaled_059 = transform_alpha_mask(&mask_base[0], 0.0, 0.59);
    assert!(
        (mask_scaled_059.width as f32 - (w_base * 0.59)).abs() <= 3.0,
        "Scaled 0.59x width must match expected: got {}, base {}",
        mask_scaled_059.width,
        w_base
    );

    // Test scale 1.208x (CapCut bounce overshoot scale factor)
    let mask_scaled_1208 = transform_alpha_mask(&mask_base[0], 0.0, 1.208);
    assert!(
        (mask_scaled_1208.width as f32 - (w_base * 1.208)).abs() <= 3.0,
        "Scaled 1.208x width must match expected: got {}, base {}",
        mask_scaled_1208.width,
        w_base
    );

    // Test rotation -21° (CapCut initial bounce angle)
    let mask_rot_21 = transform_alpha_mask(&mask_base[0], -21.0, 1.0);
    assert!(
        mask_rot_21.width > mask_base[0].width && mask_rot_21.height > mask_base[0].height,
        "Rotated mask bounding box must expand to accommodate diagonal orientation"
    );

    // Count opaque core pixels and semi-transparent shadow/stroke fringe pixels inside ROI
    let mut opaque_count = 0usize;
    let mut fringe_count = 0usize;

    // 2. White background compositing — Mathematical proof of ZERO dark halo on semi-transparent fringe
    let mut white_bg = RgbaBuffer::new(1920, 1080);
    white_bg.fill_color(255, 255, 255, 255);
    white_bg.blend_buffer_over(&overlay);

    for y in roi[1]..roi[3] {
        for x in roi[0]..roi[2] {
            let [sr, sg, sb, sa] = overlay.get_pixel(x, y);
            let [wr, wg, wb, wa] = white_bg.get_pixel(x, y);
            assert_eq!(wa, 255);
            if sa == 255 {
                opaque_count += 1;
            } else if sa > 0 {
                fringe_count += 1;
                // Under straight-alpha Porter-Duff Over on white (255, 255, 255):
                // expected_c = (src_c * sa + 255 * (255 - sa)) / 255.
                // A premultiplied-alpha dark halo bug would omit multiplying src_c or double-multiply alpha,
                // causing wr/wg/wb to drop below the linear interpolation floor.
                let a_f = sa as f32 / 255.0;
                let exp_r = (sr as f32 * a_f + 255.0 * (1.0 - a_f)).round() as i32;
                let exp_g = (sg as f32 * a_f + 255.0 * (1.0 - a_f)).round() as i32;
                let exp_b = (sb as f32 * a_f + 255.0 * (1.0 - a_f)).round() as i32;
                assert!(
                    (wr as i32 - exp_r).abs() <= 1
                        && (wg as i32 - exp_g).abs() <= 1
                        && (wb as i32 - exp_b).abs() <= 1,
                    "Semi-transparent fringe pixel at ({}, {}) has halo mismatch: got [{}, {}, {}], expected [{}, {}, {}] for src [{}, {}, {}, {}]",
                    x, y, wr, wg, wb, exp_r, exp_g, exp_b, sr, sg, sb, sa
                );
            }
        }
    }

    assert!(opaque_count > 1000, "Must have >1000 opaque glyph pixels");
    assert!(
        fringe_count > 1000,
        "Must have >1000 semi-transparent anti-aliased & blurred shadow fringe pixels"
    );

    // 3. Black background compositing
    let mut black_bg = RgbaBuffer::new(1920, 1080);
    black_bg.fill_color(0, 0, 0, 255);
    black_bg.blend_buffer_over(&overlay);
    assert_eq!(black_bg.get_pixel(0, 0), [0, 0, 0, 255]);

    // 4. Color background compositing
    let mut color_bg = RgbaBuffer::new(1920, 1080);
    color_bg.fill_color(30, 80, 160, 255);
    color_bg.blend_buffer_over(&overlay);
    assert_eq!(color_bg.get_pixel(0, 0), [30, 80, 160, 255]);

    // 5. Video frame background compositing (simulated YUV->RGB natural video gradient frame)
    let mut video_bg = RgbaBuffer::new(1920, 1080);
    for y in 0..1080 {
        let vr = ((y * 180) / 1080 + 40) as u8;
        let vg = ((y * 120) / 1080 + 60) as u8;
        let vb = 110u8;
        let row_start = (y * 1920 * 4) as usize;
        for x in 0..1920 {
            let idx = row_start + (x * 4) as usize;
            video_bg.data[idx] = vr;
            video_bg.data[idx + 1] = vg;
            video_bg.data[idx + 2] = vb;
            video_bg.data[idx + 3] = 255;
        }
    }
    video_bg.blend_buffer_over(&overlay);

    // Save cropped ROI proof images for all 4 backgrounds
    let out_dir = get_root_artifact_dir();
    fs::create_dir_all(&out_dir).expect("Create phase-28 artifacts dir");
    let pad_roi = [
        roi[0].saturating_sub(20),
        roi[1].saturating_sub(20),
        (roi[2] + 20).min(1920),
        (roi[3] + 20).min(1080),
    ];
    fs::write(
        out_dir.join("poc_alpha_white.png"),
        white_bg.crop(pad_roi).to_png_bytes(),
    )
    .expect("Write white alpha proof");
    fs::write(
        out_dir.join("poc_alpha_black.png"),
        black_bg.crop(pad_roi).to_png_bytes(),
    )
    .expect("Write black alpha proof");
    fs::write(
        out_dir.join("poc_alpha_color.png"),
        color_bg.crop(pad_roi).to_png_bytes(),
    )
    .expect("Write color alpha proof");
    fs::write(
        out_dir.join("poc_alpha_video.png"),
        video_bg.crop(pad_roi).to_png_bytes(),
    )
    .expect("Write video alpha proof");
}

#[test]
fn test_p28_ac05_workloads_w1_to_w4_benchmarks() {
    let font_bytes = get_open_source_fixture_font();
    let renderer = SharedTextRendererPoc::new(1920, 1080, font_bytes.clone());

    // --- Workload W1: Single Animated Rich Title (120 graphemes, 1080p, 2 outer strokes, 2 drop shadows) ---
    // Evaluated at t=0.0s (p=0.0), t=0.5s (p=0.5), t=1.0s (p=1.0, 100% of 120 graphemes visible), and t=3.0s (p=1.0 hold)
    let w1_text = "Tinix — Thử nghiệm tiếng Việt với tiêu đề hoạt họa phong phú để đo hiệu năng rasterization trên nền tảng Windows 11 Pro!";
    let w1_graphemes = renderer.evaluate_sample_motion(w1_text, 1.0);
    assert_eq!(
        w1_graphemes.len(),
        120,
        "W1 text must contain exact 120 grapheme clusters"
    );
    let visible_at_completion = w1_graphemes.iter().filter(|c| c.opacity > 0.99).count();
    let expected_non_space = w1_graphemes.iter().filter(|c| c.grapheme != " ").count();
    assert_eq!(
        visible_at_completion, expected_non_space,
        "At progress=1.0, all non-space graphemes ({}) must be 100% visible",
        expected_non_space
    );

    let mut buf = RgbaBuffer::new(1920, 1080);

    // Measure W1 Cold Cache at progress = 1.0 (100% characters visible)
    renderer.clear_cache();
    let t_cold_start = Instant::now();
    renderer.render_frame_into(&mut buf, w1_text, 1.0);
    let w1_cold_ms = t_cold_start.elapsed().as_secs_f64() * 1000.0;

    // Measure W1 Warm Cache across checkpoints (including p=1.0 where all 120 graphemes render)
    let iterations = 20;
    let checkpoints = [0.0f32, 0.5, 1.0, 1.0]; // t=0.0s, t=0.5s, t=1.0s, t=3.0s hold
    let mut w1_times = Vec::with_capacity(iterations);
    for i in 0..iterations {
        let p = if i < 4 { checkpoints[i] } else { 1.0 }; // 18 of 20 runs at full 120-grapheme visibility
        let start = Instant::now();
        renderer.render_frame_into(&mut buf, w1_text, p);
        w1_times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    w1_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let w1_p50 = percentile(&w1_times, 0.50);
    let w1_p95 = percentile(&w1_times, 0.95);
    println!(
        "Workload W1 (1080p, 120 graphemes, 2 strokes, 2 shadows): Cold = {:.2} ms, Warm p50 = {:.2} ms, Warm p95 = {:.2} ms",
        w1_cold_ms, w1_p50, w1_p95
    );

    // --- Workload W2: Multi-Title Concurrency (4 non-overlapping concurrent titles with background box, 100% visible) ---
    let w2_tracks = [
        ("Tiêu đề 1 — Thử nghiệm đồng thời track A", 220.0f32),
        ("Tiêu đề 2 — Thử nghiệm đồng thời track B", 430.0f32),
        ("Tiêu đề 3 — Thử nghiệm đồng thời track C", 650.0f32),
        ("Tiêu đề 4 — Thử nghiệm đồng thời track D", 860.0f32),
    ];
    let bg_box = BackgroundBoxDef {
        color: ColorRgba::new(0.08, 0.08, 0.12, 0.78),
        padding_x: 20.0,
        padding_y: 10.0,
        border_radius: 8.0,
    };

    renderer.clear_cache();
    let t_w2_cold = Instant::now();
    buf.clear();
    for (title, cy) in &w2_tracks {
        renderer.composite_layer_at_into(&mut buf, title, 1.0, Some(*cy), Some(&bg_box));
    }
    let w2_cold_ms = t_w2_cold.elapsed().as_secs_f64() * 1000.0;

    let mut w2_times = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        buf.clear();
        for (title, cy) in &w2_tracks {
            renderer.composite_layer_at_into(&mut buf, title, 1.0, Some(*cy), Some(&bg_box));
        }
        w2_times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    w2_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let w2_p50 = percentile(&w2_times, 0.50);
    let w2_p95 = percentile(&w2_times, 0.95);
    println!(
        "Workload W2 (4 non-overlapping concurrent titles + bg boxes): Cold = {:.2} ms, Warm p50 = {:.2} ms, Warm p95 = {:.2} ms",
        w2_cold_ms, w2_p50, w2_p95
    );

    // --- Workload W3: Maximum Layer Density (16 layers = 8 decorative shape layers + 8 text slots x 32 graphemes) ---
    let w3_slots: Vec<String> = (0..8)
        .map(|i| format!("Slot {}: Nội dung văn bản tầng #{}", i + 1, i + 1)) // 32 graphemes each
        .collect();
    assert_eq!(w3_slots[0].chars().count(), 32);

    let w3_shapes: Vec<ShapeLayerDef> = (0..8)
        .map(|i| ShapeLayerDef {
            x: 360.0,
            y: 80.0 + (i as f32 * 115.0),
            width: 1200.0,
            height: 76.0,
            border_radius: 10.0,
            color: ColorRgba::new(0.12, 0.16, 0.24, 0.70),
        })
        .collect();

    renderer.clear_cache();
    let t_w3_cold = Instant::now();
    buf.clear();
    for shape in &w3_shapes {
        renderer.render_shape_layer(&mut buf, shape);
    }
    for (i, slot_text) in w3_slots.iter().enumerate() {
        let cy = 118.0 + (i as f32 * 115.0);
        renderer.composite_layer_at_into(&mut buf, slot_text, 1.0, Some(cy), None);
    }
    let w3_cold_ms = t_w3_cold.elapsed().as_secs_f64() * 1000.0;

    let mut w3_times = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        buf.clear();
        for shape in &w3_shapes {
            renderer.render_shape_layer(&mut buf, shape);
        }
        for (i, slot_text) in w3_slots.iter().enumerate() {
            let cy = 118.0 + (i as f32 * 115.0);
            renderer.composite_layer_at_into(&mut buf, slot_text, 1.0, Some(cy), None);
        }
        w3_times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    w3_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let w3_p50 = percentile(&w3_times, 0.50);
    let w3_p95 = percentile(&w3_times, 0.95);
    println!(
        "Workload W3 16:9 (16 layers = 8 shapes + 8x32-char slots): Cold = {:.2} ms, Warm p50 = {:.2} ms, Warm p95 = {:.2} ms",
        w3_cold_ms, w3_p50, w3_p95
    );

    // Also test W3 on 9:16 vertical aspect ratio (1080x1920)
    let renderer_9x16 = SharedTextRendererPoc::new(1080, 1920, font_bytes);
    let mut buf_9x16 = RgbaBuffer::new(1080, 1920);
    let w3_shapes_9x16: Vec<ShapeLayerDef> = (0..8)
        .map(|i| ShapeLayerDef {
            x: 60.0,
            y: 162.0 + (i as f32 * 190.0),
            width: 960.0,
            height: 76.0,
            border_radius: 10.0,
            color: ColorRgba::new(0.12, 0.16, 0.24, 0.70),
        })
        .collect();
    let t_w3_9x16_cold = Instant::now();
    for shape in &w3_shapes_9x16 {
        renderer_9x16.render_shape_layer(&mut buf_9x16, shape);
    }
    for i in 0..8 {
        let cy = 200.0 + (i as f32 * 190.0);
        renderer_9x16.composite_layer_at_into(&mut buf_9x16, &w3_slots[i], 1.0, Some(cy), None);
    }
    let w3_9x16_cold_ms = t_w3_9x16_cold.elapsed().as_secs_f64() * 1000.0;
    let mut w3_9x16_times = Vec::with_capacity(10);
    for _ in 0..10 {
        let start = Instant::now();
        buf_9x16.clear();
        for shape in &w3_shapes_9x16 {
            renderer_9x16.render_shape_layer(&mut buf_9x16, shape);
        }
        for i in 0..8 {
            let cy = 200.0 + (i as f32 * 190.0);
            renderer_9x16.composite_layer_at_into(&mut buf_9x16, &w3_slots[i], 1.0, Some(cy), None);
        }
        w3_9x16_times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    w3_9x16_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let w3_9x16_p50 = percentile(&w3_9x16_times, 0.50);
    let w3_9x16_p95 = percentile(&w3_9x16_times, 0.95);
    println!(
        "Workload W3 9:16 (16 layers = 8 shapes + 8x32-char slots): Cold = {:.2} ms, Warm p50 = {:.2} ms, Warm p95 = {:.2} ms",
        w3_9x16_cold_ms, w3_9x16_p50, w3_9x16_p95
    );

    // Measure Seek Latency (non-monotonic jumps across timeline progress: 0.1 -> 0.9 -> 0.3 -> 0.75 -> 0.05)
    let seek_targets = [0.10f32, 0.90, 0.30, 0.75, 0.05];
    let mut seek_times = Vec::with_capacity(seek_targets.len());
    for &target_p in &seek_targets {
        let t_seek = Instant::now();
        buf.clear();
        renderer.render_frame_into(&mut buf, w1_text, target_p);
        seek_times.push(t_seek.elapsed().as_secs_f64() * 1000.0);
    }
    seek_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let seek_p50 = percentile(&seek_times, 0.50);
    let seek_p95 = percentile(&seek_times, 0.95);
    println!(
        "Seek Latency (random timeline jumps): p50 = {:.2} ms, p95 = {:.2} ms",
        seek_p50, seek_p95
    );

    // Measure Edit Latency (text edit invalidation and re-render)
    let edited_texts = [
        "Tinix — Cập nhật tiêu đề lần 1!",
        "Tinix — Biên tập văn bản hoạt họa thời gian thực!",
        "Tinix — Hoàn tất kiểm thử chỉnh sửa!",
    ];
    let mut edit_times = Vec::with_capacity(edited_texts.len());
    for edited in &edited_texts {
        let t_edit = Instant::now();
        buf.clear();
        renderer.render_frame_into(&mut buf, edited, 1.0);
        edit_times.push(t_edit.elapsed().as_secs_f64() * 1000.0);
    }
    edit_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let edit_p50 = percentile(&edit_times, 0.50);
    let edit_p95 = percentile(&edit_times, 0.95);
    println!(
        "Edit Latency (text mutation & re-render): p50 = {:.2} ms, p95 = {:.2} ms",
        edit_p50, edit_p95
    );

    // Measure Preview IPC End-to-End Delivery Latency (render + dirty ROI crop + PNG + JSON serialization)
    let mut ipc_delivery_times = Vec::with_capacity(10);
    for _ in 0..10 {
        let t_ipc = Instant::now();
        let resp = invoke_poc_preview_ipc(w1_text, 0.5);
        let _ = serde_json::to_vec(&resp).expect("JSON serialize");
        ipc_delivery_times.push(t_ipc.elapsed().as_secs_f64() * 1000.0);
    }
    ipc_delivery_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let ipc_p50 = percentile(&ipc_delivery_times, 0.50);
    let ipc_p95 = percentile(&ipc_delivery_times, 0.95);
    println!(
        "Preview IPC Delivery (end-to-end): p50 = {:.2} ms, p95 = {:.2} ms",
        ipc_p50, ipc_p95
    );

    // Verify Bounded Overlay Cache Memory Ceiling (<= 256 MiB)
    let cache_bytes = renderer.cache_bytes_used();
    let (entries, hits, misses) = renderer.cache_stats();
    println!(
        "Bounded Glyph Cache: {} bytes ({:.2} MiB / 256.00 MiB max), entries = {}, hits = {}, misses = {}",
        cache_bytes,
        cache_bytes as f64 / (1024.0 * 1024.0),
        entries,
        hits,
        misses
    );
    assert!(
        cache_bytes > 0 && cache_bytes <= DEFAULT_CACHE_MAX_BYTES,
        "Cache memory usage must stay strictly within 256 MiB ceiling"
    );
    assert!(hits > misses, "Warm cache must record high cache hit ratio");

    // Write structured CSV benchmark report. Runs always write a run-scoped file so the
    // historical gate evidence is never overwritten by a failing or simply different host.
    let out_dir = get_root_artifact_dir();
    fs::create_dir_all(&out_dir).expect("Create phase-28 dir");
    let status = |value: f64, budget: f64| if value <= budget { "PASS" } else { "FAIL" };
    let csv_content = format!(
        "workload,resolution,cold_ms,warm_p50_ms,warm_p95_ms,budget_p95_ms,cache_bytes,status\n\
         W1_Single_120_Graphemes_2Stroke_2Shadow,1920x1080,{:.2},{:.2},{:.2},16.00,{},{}\n\
         W2_4_Concurrent_Titles_NonOverlapping,1920x1080,{:.2},{:.2},{:.2},33.30,{},{}\n\
         W3_16_Layers_8Shapes_8TextSlots_16x9,1920x1080,{:.2},{:.2},{:.2},50.00,{},{}\n\
         W3_16_Layers_Vertical_9x16,1080x1920,{:.2},{:.2},{:.2},50.00,{},{}\n\
         W1_Preview_IPC_Delivery_EndToEnd,1920x1080,0.00,{:.2},{:.2},16.00,{},{}\n\
         Seek_Latency_Random_Timeline_Jumps,1920x1080,0.00,{:.2},{:.2},16.00,{},{}\n\
         Edit_Latency_Text_Mutation,1920x1080,0.00,{:.2},{:.2},25.00,{},{}\n",
        w1_cold_ms,
        w1_p50,
        w1_p95,
        cache_bytes,
        status(w1_p95, 16.0),
        w2_cold_ms,
        w2_p50,
        w2_p95,
        cache_bytes,
        status(w2_p95, 33.3),
        w3_cold_ms,
        w3_p50,
        w3_p95,
        cache_bytes,
        status(w3_p95, 50.0),
        w3_9x16_cold_ms,
        w3_9x16_p50,
        w3_9x16_p95,
        renderer_9x16.cache_bytes_used(),
        status(w3_9x16_p95, 50.0),
        ipc_p50,
        ipc_p95,
        cache_bytes,
        status(ipc_p95, 16.0),
        seek_p50,
        seek_p95,
        cache_bytes,
        status(seek_p95, 16.0),
        edit_p50,
        edit_p95,
        cache_bytes,
        status(edit_p95, 25.0)
    );
    let runs_dir = out_dir.join("runs");
    fs::create_dir_all(&runs_dir).expect("Create phase-28 runs dir");
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Clock after epoch")
        .as_millis();
    let run_path = runs_dir.join(format!("benchmark-{run_id}.csv"));
    fs::write(&run_path, &csv_content).expect("Write run-scoped benchmark CSV");
    println!("Phase 28 benchmark run evidence: {}", run_path.display());
    let all_pass = w1_p95 <= 16.0 && w2_p95 <= 33.3 && w3_p95 <= 50.0 && w3_9x16_p95 <= 50.0;
    if all_pass
        && std::env::var("TINIX_P28_UPDATE_GATE_EVIDENCE")
            .ok()
            .as_deref()
            == Some("1")
    {
        fs::write(out_dir.join("benchmark-metrics.csv"), &csv_content)
            .expect("Write canonical benchmark-metrics.csv");
        println!("Updated canonical gate evidence (TINIX_P28_UPDATE_GATE_EVIDENCE=1)");
    }

    // Assert that all latencies satisfy Phase 27 locked budgets
    assert!(
        w1_p95 <= 16.0,
        "W1 Warm p95 must satisfy budget <= 16.0 ms, got {:.2} ms",
        w1_p95
    );
    assert!(
        w2_p95 <= 33.3,
        "W2 Warm p95 must satisfy budget <= 33.3 ms, got {:.2} ms",
        w2_p95
    );
    assert!(
        w3_p95 <= 50.0 && w3_9x16_p95 <= 50.0,
        "W3 Warm p95 must satisfy budget <= 50.0 ms, got {:.2} ms (16:9) / {:.2} ms (9:16)",
        w3_p95,
        w3_9x16_p95
    );
}

#[test]
fn test_p28_ac06_ffmpeg_export_stream_and_cancellation() {
    let font_bytes = get_open_source_fixture_font();
    let renderer = SharedTextRendererPoc::new(1920, 1080, font_bytes);
    let ffmpeg_bin = get_ffmpeg_bin();

    if !ffmpeg_bin.exists() {
        println!(
            "Skipping FFmpeg stream test: ffmpeg.exe not found at {:?}",
            ffmpeg_bin
        );
        return;
    }

    let out_dir = get_root_artifact_dir();
    fs::create_dir_all(&out_dir).expect("Create phase-28 dir");
    let out_mp4 = out_dir.join("poc_export_sample.mp4");

    // 1. Measure real FFmpeg stream export over solid background (2.0s, 30 fps = 60 frames)
    let cancel_token = Arc::new(AtomicBool::new(false));
    let t_start = Instant::now();
    let export_res = renderer.stream_to_ffmpeg_overlay(
        &ffmpeg_bin,
        "color=black",
        "Tinix — Thử nghiệm tiếng Việt",
        2.0,
        30,
        &out_mp4,
        cancel_token.clone(),
    );
    let elapsed = t_start.elapsed().as_secs_f64();
    let rtf = 2.0 / elapsed; // Real-time factor: 2.0s video duration / elapsed
    println!(
        "FFmpeg export (W4): {:.3}s for 2.0s (60 frames) video -> {:.2}x RTF",
        elapsed, rtf
    );

    assert!(
        export_res.is_ok(),
        "FFmpeg export stream must succeed: {:?}",
        export_res.err()
    );
    let summary = export_res.unwrap();
    assert_eq!(summary.total_frames, 60);
    assert!(out_mp4.exists());
    assert!(
        summary.file_size_bytes > 10_000,
        "Exported MP4 must be valid video file"
    );
    assert!(
        rtf >= 1.5,
        "W4 export throughput must be >= 1.5x real-time, got {:.2}x",
        rtf
    );

    // 2. Verify encoded MP4 visual quality separately (extract last frame via FFmpeg and compare ROI SSIM)
    let raw_out = Command::new(&ffmpeg_bin)
        .args([
            "-v",
            "error",
            "-i",
            out_mp4.to_str().unwrap(),
            "-vf",
            "select=eq(n\\,59)",
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])
        .output()
        .expect("Extract final frame from MP4");

    if raw_out.status.success() && raw_out.stdout.len() == 1920 * 1080 * 4 {
        let mp4_frame = RgbaBuffer {
            width: 1920,
            height: 1080,
            data: raw_out.stdout,
        };
        let mut expected_black_comp = RgbaBuffer::new(1920, 1080);
        expected_black_comp.fill_color(0, 0, 0, 255);
        let overlay_end = renderer.render_frame("Tinix — Thử nghiệm tiếng Việt", 1.0);
        let roi = overlay_end.compute_dirty_roi().unwrap();
        expected_black_comp.blend_buffer_over(&overlay_end);

        let mp4_ssim = compute_roi_ssim(&mp4_frame, &expected_black_comp, roi);
        println!(
            "Encoded H.264 MP4 frame ROI SSIM vs lossless composite: {:.4}",
            mp4_ssim
        );
        assert!(
            mp4_ssim >= 0.90,
            "Encoded MP4 frame must preserve high perceptual ROI similarity (>= 0.90), got {:.4}",
            mp4_ssim
        );
    }

    // 3. Test Cancellation: Cancel token triggered immediately
    let cancel_mp4 = out_dir.join("poc_cancelled.mp4");
    let cancel_token_active = Arc::new(AtomicBool::new(true));
    let cancel_res = renderer.stream_to_ffmpeg_overlay(
        &ffmpeg_bin,
        "color=black",
        "Tinix — Cancelled test",
        2.0,
        30,
        &cancel_mp4,
        cancel_token_active,
    );

    assert!(cancel_res.is_err(), "Must report cancellation error");
    assert!(
        !cancel_mp4.exists(),
        "Cancelled file must be removed cleanly"
    );
}
