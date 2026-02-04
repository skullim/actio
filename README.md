# Actio

Rust framework for long‑running IO bound async tasks that produce a stream of feedback, support cancellation, and eventually resolve to a terminal outcome.

Feedback and cancellation are selected at compile time, so you don’t pay for features you don’t opt in to.

### Features

- **Feedback streams** - Progress updates while the action runs. Implemented by default via `tokio::watch` channel, however user is free to implement `FeedbackReceiverMarker` to integrate 3rd party channels (and use corresponding `Sender` part in `ServerConcept` implementation).
- **Cancellation** - Graceful cancellation with optional context
- **State Snapshots** - Capture task state at cancellation time
- **Stateful servers** - Visit the server and update its internal state based on Outcome


### Comparison with ROS actions

`actio` aims to provide functionality similar to ROS 2 actions (goal, feedback, result), but with a different set of tradeoffs.
​

In-process lifecycle: `actio` keeps the entire action lifecycle in the same process, avoiding the ROS 2 action transport endpoints (topics like `/action/status` and `/action/feedback`, plus services like `/action/send_goal`, `/action/cancel_goal`, and `/action/get_result`) as well as the associated discovery and (de)serialization costs.
​

Typed handles instead of UUIDs: ROS 2 uses a goal ID (UUID) to identify goals across distributed clients; `actio` can expose strongly typed goal/task handles because the client and server share an address space.
​

Cancellation semantics: In ROS 2, cancellation is requested via the `/action/cancel_goal` service, and whether a goal ultimately transitions to `CANCELED` is indicated via the status topic and the result service. In `actio`, cancellation can be modeled as an in-process signal/event and paired with a “visit/update state” mechanism to keep server logic clean.
​

To sum up, if you want long-running async tasks with a client↔server shape but you don’t need a distributed system boundary, `actio` can be a good fit.


### Design Diagram
![Design Diagram](docs/design.excalidraw.png)


### Runtime
`actio` currently depends on `tokio`, however if needed some work might be dedicated to try to make it runtime agnostic.