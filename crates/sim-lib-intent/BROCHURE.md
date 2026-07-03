# sim-lib-intent

In one line: it turns a click, drag, or typed edit into a clear request the system can check before anything changes.

## What it gives you

Every time you act on a value in the browser -- moving a node, setting a field, wiring two things together -- your gesture becomes a small, readable record of what you wanted. That record says who asked (a person or an agent), when, and against what. The system checks the request before it touches live state, so a bad edit is caught and explained instead of silently going through. The same record works whether the action came from your mouse or from an automated helper. Nothing is guessed from raw pixels; the intent is stated plainly and kept for review.

## Why you will be glad

- An edit that does not make sense is stopped early, with a reason you can read.
- Your actions and an agent's actions are recorded the same clear way.
- Every change carries who did it and when, so you can trace any edit later.

## Where it fits

This is the front door for editing in the SIM browser workspace. It sits between the raw actions you make on screen and the runtime that holds the real values. Views show you a value; this crate captures what you want to do to it and hands a checked request onward. It is the shared language of edits that the rest of the view stack builds on.
