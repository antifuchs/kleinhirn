# kleinhirn - the 70% of Einhorn that are useful in a container context

Kleinhirn is a preloading process supervisor with worker
acknowledgement targeting Linux. It supports Ruby with preloading out
of the box, and can also run regular UNIX programs using plain
fork/exec.

## Things kleinhirn doesn't do

Not all the things einhorn does are useful in a container context (and
are done better by other tools), so kleinhirn doesn't even
attempt to do them. Here they are:

* In-place upgrades - kleinhirn expects that you have a deploy
  strategy that involves a load balancer and (potentially) blue/green
  deploys. It'll tell you the state of your processes, so you can hook
  into e.g. k8s liveness checks, though.

* Listen socket handling - you can easily open one or multiple
  listening sockets by invoking kleinhirn with already-opened sockets
  with [`systemfd`](https://github.com/mitsuhiko/systemfd) or other
  tools such as
  [`tcp-socket-listen`](https://jdebp.eu/Softwares/nosh/guide/commands/tcp-socket-listen.xml)
  from [the nosh suite](https://jdebp.eu/Softwares/nosh). These tools
  are more flexible than any built-in socket listening scheme could
  be, and you are guaranteed that your socket remains available across
  restarts of the supervisor.

* Runtime control - There is no `kleinhirnsh` program and no control
  socket that you can use to give kleinhirn commands to, say, spawn a
  new worker or to kick off an upgrade. Similar to the "no upgrades"
  stance, the expectation is that you run kleinhirn in an immutable
  container - replace the container to reconfigure your process
  situation.

* Support for any OS other than Linux - kleinhirn sets itself up as a
  "child subreaper", which allows it to pretend to "be init", a
  functionality that's only available on Linux kernels (which it needs
  to properly monitor processes under preloading). I assume you were
  planning to run it with containers under Linux.

## Things kleinhirn does do

* Spawns your processes: kleinhirn allows using fork/exec to spawn
  programs, and code preloading in Ruby, using the included
  `kleinhirn_loader` gem.

* Supervises your processes: kleinhirn ensures that the configured
  number of processes remains running, with configurable thresholds
  when "too many" worker deaths have occurred in a period of time.

* Reports health: There is a configurable HTTP endpoint that
  orchestrators can query to figure out if the worker set is fully
  spawned and healthy/ready to serve requests. If workers die too
  quickly in succession (configurably), the worker set is marked
  unhealthy, which should help you monitor your service health.

* Collects acks from your workers: To appropriately reflect the state
  of your program, kleinhirn collects information from worker
  processes on when they're ready to actually serve a request. If they
  don't ack in a configurable timespan, they are marked as broken, and
  the worker set turns unhealthy.

* Structured logging: kleinhirn logs to stderr or stdout
  (configurable), using [`logfmt`](https://brandur.org/logfmt) format,
  but you can switch it to JSON too.
