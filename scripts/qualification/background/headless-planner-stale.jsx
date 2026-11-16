import React, { useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { mockIPC } from '@tauri-apps/api/mocks';
import { EditPlanTrimDialog } from '../../../src/components/EditPlanTrimDialog';
import { EditPlanReorderDialog } from '../../../src/components/EditPlanReorderDialog';
import { SceneGuidedDraftDialog } from '../../../src/components/SceneGuidedDraftDialog';
import { AudioMixDraftDialog } from '../../../src/components/AudioMixDraftDialog';

const rate = { numerator: 30, denominator: 1 };
const item = (id, kind, start) => ({ item_id: id, asset_id: id, kind, name: id, volume: 1, embedded_audio_enabled: true, timeline_range: { start: { value: start, rate }, duration: { value: 90, rate } }, source_range: { start: { value: 0, rate }, duration: { value: 90, rate } }, keyframes: [] });
const report = { schemaVersion: 1, algorithmVersion: 'test', provider: 'test', model: 'test', query: 'test', sourceFingerprint: 'test', configFingerprint: 'test', truncated: false, warnings: [], scenes: [{ sceneId: 's1', assetId: 'a', assetName: 'a', sourceStartMicros: 0, sourceEndMicros: 3000000, sampledAtMicros: 0, caption: 'test', tags: [], relevanceScore: 1, rationale: '' }] };
mockIPC((command, args) => window.plannerInvoke(command, args));
function Harness() {
  const [revision, setRevision] = useState(1);
  const [offline, setOffline] = useState({});
  const project = useMemo(() => ({ project_id: 'p', revision, schema_version: 16, assets: [{ asset_id: 'a', media_kind: 'Video' }, { asset_id: 'b', media_kind: 'Video' }, { asset_id: 'music', media_kind: 'Audio' }], settings: { frame_rate: rate }, tracks: [{ track_id: 'v', kind: 'Video', name: 'Video', items: [item('a', 'Video', 0), item('b', 'Video', 90)] }, { track_id: 'm', kind: 'Audio', name: 'Music', items: [item('music', 'Audio', 0)] }] }), [revision]);
  const mode = new URLSearchParams(location.search).get('mode');
  const common = { project, offlineMedia: offline, onClose: () => {}, onApply: async () => { window.applied++; } };
  return <><button onClick={() => setRevision(value => value + 1)}>Change revision</button><button onClick={() => setOffline({a:{last_known_path:'a',permission_required:false}})}>Offline a</button><button onClick={() => setOffline({})}>Online all</button><button onClick={() => setOffline(current => ({...current}))}>Refresh media</button>
    {mode === 'trim' && <EditPlanTrimDialog {...common} isOpen item={project.tracks[0].items[0]} />}
    {mode === 'reorder' && <EditPlanReorderDialog {...common} isOpen selectedItemIds={[]} />}
    {mode === 'scene' && <SceneGuidedDraftDialog {...common} report={report} />}
    {mode === 'mix' && <AudioMixDraftDialog {...common} isOpen />}
  </>;
}
window.applied = 0;
createRoot(document.getElementById('root')).render(<Harness />);
