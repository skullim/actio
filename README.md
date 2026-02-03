# Actio

Rust framework for long‑running async actions that produce a stream of feedback, support cancellation, and eventually resolve to a terminal result.

Feedback and cancellation are selected at compile time, so you don’t pay for features you don’t opt in to.

### Features

- **Feedback streams** - Progress updates while the action runs 
- **Cancellation** - Graceful cancellation with optional context
- **State Snapshots** - Capture task state at cancellation time
- **Stateful servers** - Visit the server and update its internal state based on Outcome


### Comparison with ROS actions

`actio` aims to provide functionality similar to ROS 2 actions (goal, feedback, result), but with a different set of tradeoffs. ROS 2 actions are explicitly designed as a communication primitive for long-running tasks and consist of a goal, feedback, and a result.
​

In-process lifecycle: `actio` keeps the entire action lifecycle in the same process, avoiding the ROS 2 action transport endpoints (topics like .../_action/status and .../_action/feedback, plus services like .../_action/send_goal, .../_action/cancel_goal, and .../_action/get_result) as well as the associated discovery and (de)serialization costs.
​

Typed handles instead of UUIDs: ROS 2 uses a goal ID (UUID) to identify goals across distributed clients; `actio` can expose strongly typed goal/task handles because the client and server share an address space.
​

Cancellation semantics: In ROS 2, cancellation is requested via the .../_action/cancel_goal service, and whether a goal ultimately transitions to CANCELED is indicated via the status topic and the result service. In `actio`, cancellation can be modeled as an in-process signal/event and paired with a “visit/update state” mechanism to keep server logic clean (tasks still must be cancellation-safe).
​

To sum up, if you want long-running async tasks with a client↔server shape but you don’t need a distributed system boundary, `actio` can be a good fit.


### Design Diagram
![Design Diagram](docs/design.excalidraw.png)
