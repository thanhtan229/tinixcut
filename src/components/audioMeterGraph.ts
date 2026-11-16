export function createAudioMeterBus(context: BaseAudioContext) {
  const analyser = context.createAnalyser();
  analyser.fftSize = 1024;
  const silentGain = context.createGain();
  silentGain.gain.value = 0;
  analyser.connect(silentGain);
  silentGain.connect(context.destination);
  return { analyser, silentGain, samples: new Float32Array(new ArrayBuffer(analyser.fftSize)) };
}

export function connectAudioMeterTap(context: BaseAudioContext, source: AudioNode, bus: AnalyserNode) {
  const gain = context.createGain();
  const analyser = context.createAnalyser();
  analyser.fftSize = 1024;
  source.connect(gain);
  gain.connect(analyser);
  analyser.connect(bus);
  return { gain, analyser, samples: new Float32Array(new ArrayBuffer(analyser.fftSize)) };
}
