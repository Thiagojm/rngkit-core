# rngkit-engine

Synchronous, caller-owned, cancellable single-source collection.

The engine does not own a runtime or worker thread. Each cycle is measured with
a monotonic clock from before acquisition through durable recording, analysis,
and successful event delivery. The wait is `interval.saturating_sub(elapsed)`.
Overruns emit `TimingOverrun` and start the next read immediately. Cancellation
during a wait is prompt; cancellation during a successful blocking read commits
that complete sample and then stops.
