import { connectAudioMeterTap, createAudioMeterBus } from './audioMeterGraph';

// One source per element for its entire lifetime. Disconnecting and recreating a
// MediaElementAudioSourceNode for the same element is not supported by Web Audio.
export class PreviewAudioOutput {
  readonly bus;
  readonly outputs = new Map<HTMLMediaElement, ReturnType<PreviewAudioOutput['connect']>>();

  constructor(readonly context: AudioContext) {
    this.bus = createAudioMeterBus(context);
  }

  private connect(element: HTMLMediaElement) {
    const source = this.context.createMediaElementSource(element);
    const tap = connectAudioMeterTap(this.context, source, this.bus.analyser);
    tap.gain.gain.value = 0;
    // The element's automatic output is replaced by createMediaElementSource.
    // There is one audible edge; the bus observation path ends at zero gain.
    tap.gain.connect(this.context.destination);
    return { ...tap, source };
  }

  setGain(element: HTMLMediaElement, gain: number) {
    let output = this.outputs.get(element);
    if (!output) {
      output = this.connect(element);
      this.outputs.set(element, output);
    }
    element.volume = 1;
    element.muted = false;
    output.gain.gain.value = Number.isFinite(gain) ? Math.max(0, gain) : 0;
    return output;
  }

  silence() {
    for (const output of this.outputs.values()) output.gain.gain.value = 0;
  }

  retain(elements: ReadonlySet<HTMLMediaElement>) {
    for (const [element, output] of this.outputs) {
      if (elements.has(element)) continue;
      element.pause();
      output.source.disconnect(); output.gain.disconnect(); output.analyser.disconnect();
      this.outputs.delete(element);
    }
  }

  dispose() {
    this.retain(new Set());
    this.bus.analyser.disconnect(); this.bus.silentGain.disconnect();
  }
}
