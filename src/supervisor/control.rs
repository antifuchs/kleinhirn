pub(crate) enum Command {
    /// Perform a "safe" upgrade - load the new code and replace existing children.
    Upgrade,

    /// Terminate the entire hierarchy - signal all children to exit, and exit once they have quit.
    Quit,
}
