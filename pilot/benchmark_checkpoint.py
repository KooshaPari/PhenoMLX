"""Atomic benchmark checkpoints that cannot mistake one run for a completed A/B."""
import json
import os
from pathlib import Path
import tempfile


def write_checkpoint(output, report):
    """Publish final filename only when both ten-prompt conditions are present."""
    runs = report.get('runs', {})
    complete = all(len(runs.get(key, {}).get('prompts', [])) == 10
                   for key in ('A_fp16', 'B_qdq4'))
    output = Path(output)
    target = output if complete else output.with_suffix('.partial.json')
    payload = dict(report, completion_status='complete' if complete else 'partial')
    target.parent.mkdir(parents=True, exist_ok=True)
    name = None
    try:
        with tempfile.NamedTemporaryFile(mode='w', encoding='utf-8',
                                         dir=target.parent, delete=False) as stream:
            name = stream.name
            json.dump(payload, stream, indent=2, allow_nan=False)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(name, target)
    finally:
        if name and os.path.exists(name):
            os.unlink(name)
    return target
