import { describe, expect, it } from "vitest";
import { buildSilenceCutOperations, SilenceAnalysis, silenceKeepRanges } from "./silenceCuts";
import { Asset } from "../types/project";
const analysis: SilenceAnalysis = {sourceFingerprint:"x",durationMicros:4e6,config:{thresholdDbfs:-40,minimumSilenceMs:500,paddingMs:100},measurement:"test",cuts:[{startMicros:1.1e6,endMicros:1.9e6},{startMicros:3.1e6,endMicros:3.9e6}]};
const rate={numerator:30,denominator:1};
const asset={asset_id:"source",name:"voice.wav",media_kind:"Audio",duration:{value:120,rate}} as Asset;
describe("reviewed audio silence cuts",()=>{
 it("complements only selected intervals and preserves unselected pauses",()=>{
  expect(silenceKeepRanges(analysis,new Set([0]),rate)).toEqual({totalFrames:120,removedFrames:24,ranges:[{sourceInSeconds:0,sourceOutSeconds:1.1},{sourceInSeconds:1.9,sourceOutSeconds:4}]});
 });
 it("rounds cuts inward at fractional rate and never cuts outside detected boundaries",()=>{
  const fps=30000/1001;const result=silenceKeepRanges(analysis,new Set([0]),{numerator:30000,denominator:1001});
  expect(result.ranges[0].sourceOutSeconds).toBeGreaterThanOrEqual(1.1);expect(result.ranges[1].sourceInSeconds).toBeLessThanOrEqual(1.9);
  expect(result.removedFrames).toBe(Math.floor(1.9*fps)-Math.ceil(1.1*fps));
 });
 it("builds a new-track transaction with consecutive kept source ranges; source is unchanged",()=>{
  const before=JSON.stringify({asset,analysis});const ops=buildSilenceCutOperations(asset,analysis,new Set([0,1]),rate,60,77);
  expect(ops).toHaveLength(4);expect(ops[0].operation).toMatchObject({type:"TrackCreate",payload:{name:"Silence Cut · voice.wav",kind:"Audio"}});
  expect(ops[1].operation).toMatchObject({type:"ClipCreate",payload:{source_start:{value:0},duration:{value:33},timeline_start:{value:60}}});
  expect(ops[2].operation).toMatchObject({type:"ClipCreate",payload:{source_start:{value:57},duration:{value:36},timeline_start:{value:93}}});
  expect(JSON.stringify({asset,analysis})).toBe(before);
 });
 it("rejects no cuts, all-silent full removal, malformed and overlapping analysis",()=>{
  expect(()=>buildSilenceCutOperations(asset,analysis,new Set(),rate,0)).toThrow(/at least one/);
  expect(()=>buildSilenceCutOperations(asset,analysis,new Set([99]),rate,0)).toThrow(/stale/);
  expect(()=>buildSilenceCutOperations(asset,{...analysis,cuts:[{startMicros:0,endMicros:4e6}]},new Set([0]),rate,0)).toThrow(/entire source/);
  expect(()=>silenceKeepRanges({...analysis,cuts:[{startMicros:0,endMicros:5e6}]},new Set([0]),rate)).toThrow(/Invalid/);
  expect(()=>silenceKeepRanges({...analysis,cuts:[analysis.cuts[0],analysis.cuts[0]]},new Set([0]),rate)).toThrow(/overlapping/);
 });
 it("keeps short sub-frame detections and creates video rather than audio for video sources",()=>{
  const tiny={...analysis,cuts:[{startMicros:1,endMicros:100}]};expect(silenceKeepRanges(tiny,new Set([0]),rate).removedFrames).toBe(0);
  expect(buildSilenceCutOperations({...asset,media_kind:"Video"},analysis,new Set([0]),rate,0)[1].operation).toMatchObject({payload:{kind:"Video"}});
 });
});
