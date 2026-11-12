"""Read the timeline as the UI presents it: clips (from accessible labels), revision,
status line, playhead and selection. Prints JSON.  Usage: python -I timeline_state.py"""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import evaluate

STATE = r"""(() => {
  const t = document.body.innerText;
  const clips = [...document.querySelectorAll('[data-timeline-clip]')].map(e => {
    const l = e.getAttribute('aria-label');
    const m = l.match(/^(.*), (\w+) clip(, selected)?, starts at frame (\d+), duration (\d+) frames$/);
    const lane = e.closest('[data-track-lane]');
    return m ? {name: m[1], kind: m[2], selected: !!m[3], start: +m[4], duration: +m[5], lane: lane ? lane.getAttribute('data-track-lane') : null} : {label: l};
  });
  return {
    revision: +((t.match(/Revision: (\d+)/) || [0, -1])[1]),
    status: (t.match(/Status: ([^\n]*)/) || [0, null])[1],
    playhead: +((t.match(/\((\d+)f\)/) || [0, -1])[1]),
    clips,
  };
})()"""

if __name__ == "__main__":
    print(json.dumps(evaluate(STATE), indent=1))
