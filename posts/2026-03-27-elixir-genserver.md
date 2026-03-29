---
title: GenServer Demystified
date: 2026-03-27
description: Understanding Elixir's GenServer without the jargon
---

GenServer is just a process that holds state and responds to messages. That's it. No magic.

## The Mental Model

Think of a GenServer as a loop:

1. It starts with some initial state
2. It waits for a message
3. It processes the message, possibly updating state
4. It goes back to step 2

```elixir
defmodule Counter do
  use GenServer

  # Client API

  def start_link(initial \\ 0) do
    GenServer.start_link(__MODULE__, initial, name: __MODULE__)
  end

  def increment, do: GenServer.cast(__MODULE__, :increment)

  def value, do: GenServer.call(__MODULE__, :value)

  # Server callbacks

  @impl true
  def init(initial), do: {:ok, initial}

  @impl true
  def handle_cast(:increment, count), do: {:noreply, count + 1}

  @impl true
  def handle_call(:value, _from, count), do: {:reply, count, count}
end
```

## Call vs Cast

Two ways to send messages:

- **call** is synchronous. You send a message and wait for a reply. Use it when you need a value back.
- **cast** is asynchronous. You send a message and move on. Use it for fire-and-forget operations.

```elixir
# This blocks until the server replies
count = Counter.value()

# This returns :ok immediately
Counter.increment()
```

## When to Use GenServer

Use it when you need:

- **Mutable state** that survives between function calls
- **Serialized access** to a shared resource (one message at a time)
- **A process** that can be supervised and restarted on failure

Don't use it when a simple function will do. Not everything needs to be a process.

## The Supervision Tree

GenServers become powerful when supervised:

```elixir
children = [
  {Counter, 0},
  {Cache, []},
  {RateLimiter, max_requests: 100}
]

Supervisor.start_link(children, strategy: :one_for_one)
```

If `Counter` crashes, the supervisor restarts it. `Cache` and `RateLimiter` keep running. This is how Erlang/Elixir achieves fault tolerance without try/catch everywhere.

## The Bottom Line

GenServer is a pattern, not a framework. Once you see it as "a process with state that handles messages," the API becomes obvious. The six callbacks (`init`, `handle_call`, `handle_cast`, `handle_info`, `terminate`, `code_change`) are just hooks into that loop.

Start simple. Add complexity only when the system demands it.
