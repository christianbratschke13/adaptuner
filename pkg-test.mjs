import fs from 'node:fs';
import { AdaptunerEngine, initSync } from './pkg/adaptuner.js';

const wasm = fs.readFileSync('./pkg/adaptuner_bg.wasm');

initSync({ module: wasm });

const engine = new AdaptunerEngine();

const result = engine.process([
    {
        time: 0,
        channel: 0,
        message: {
            type: 'noteOn',
            note: 60,
            velocity: 100,
        },
    },
    {
        time: 500000,
        channel: 0,
        message: {
            type: 'noteOn',
            note: 64,
            velocity: 100,
        },
    },
    {
        time: 1000000,
        channel: 0,
        message: {
            type: 'noteOff',
            note: 60,
            velocity: 0,
        },
    },
    {
        time: 1000000,
        channel: 0,
        message: {
            type: 'noteOff',
            note: 64,
            velocity: 0,
        },
    },
]);

console.dir(result, { depth: null });