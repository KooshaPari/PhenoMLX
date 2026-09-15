#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import { relative, resolve } from 'node:path';

const docsRoot = resolve(new URL('..', import.meta.url).pathname);
const traceabilityPath = resolve(docsRoot, 'operations/journey-traceability.md');
const text = readFileSync(traceabilityPath, 'utf8');

const allowedTypes = new Set(['annotated-screenshot', 'recording-mp4', 'recording-gif', 'animated-gif', 'journey-eval']);
const stubPattern = /<!-- RICH-MEDIA-STUB\s+([^>]*)-->/g;
const stubs = [];
const failures = [];

function parseAttrs(raw) {
  const attrs = {};
  for (const match of raw.matchAll(/([a-z-]+)="([^"]*)"/g)) {
    attrs[match[1]] = match[2];
  }
  return attrs;
}

function lineFor(index) {
  return text.slice(0, index).split('\n').length;
}

for (const match of text.matchAll(stubPattern)) {
  const attrs = parseAttrs(match[1]);
  const line = lineFor(match.index);
  const after = text.slice(match.index + match[0].length).split('\n').slice(0, 4).join('\n');

  for (const key of ['type', 'subject', 'journey', 'status']) {
    if (!attrs[key]) failures.push(`line ${line}: missing ${key} attribute`);
  }

  if (attrs.type && !allowedTypes.has(attrs.type)) {
    failures.push(`line ${line}: unsupported type ${attrs.type}`);
  }

  if (attrs.status && !['TODO', 'CAPTURED', 'PUBLISHED'].includes(attrs.status)) {
    failures.push(`line ${line}: unsupported status ${attrs.status}`);
  }

  if (!/!\[[^\]]+\]\([^)]+\)/.test(after)) {
    failures.push(`line ${line}: expected image/media markdown immediately after stub`);
  }

  stubs.push({ line, ...attrs });
}

if (stubs.length === 0) {
  failures.push('no RICH-MEDIA-STUB markers found');
}

const journeys = new Set(stubs.map((stub) => stub.journey).filter(Boolean));
const missingJourneyNames = [...journeys].filter((journey) => !text.includes(journey));
for (const journey of missingJourneyNames) {
  failures.push(`journey ${journey}: not referenced elsewhere in ${relative(docsRoot, traceabilityPath)}`);
}

const requirements = ['FR-HWL-CAPACITY-001', 'FR-HWL-FLEET-001', 'FR-HWL-INFERENCE-001'];
for (const requirement of requirements) {
  if (!text.includes(requirement)) {
    failures.push(`missing requirement trace ${requirement}`);
  }
}

if (failures.length > 0) {
  console.error('Rich media stub validation failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(`Validated ${stubs.length} rich media stubs across ${journeys.size} journeys.`);
for (const stub of stubs) {
  console.log(`- ${stub.journey}: ${stub.type} (${stub.status})`);
}
