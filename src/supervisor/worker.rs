pub(crate) struct ID(u64);

type Version = usize;

pub(crate) enum Event {
    /// A preload process at the given version is getting started on pre-loading code.  
    Loading(Version),

    /// The preload process is ready to spawn children.
    Loaded,

    /// The preload process has spawned a child and we should expect to see it register (or it
    /// has registered already).
    Spawned(Version, nix::unistd::Pid),

    /// The child at the given version is ready to serve requests
    Ready(Version, nix::unistd::Pid),
}
