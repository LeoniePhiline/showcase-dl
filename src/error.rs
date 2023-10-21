use color_eyre::eyre::Result;

use crate::ui::Ui;

pub(crate) fn color_eyre_install() -> Result<()> {
    // Replace the default `color_eyre::install()?` panic and error hooks.
    // The new hooks release the captured terminal first. This prevents garbled backtrace prints.
    let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default().into_hooks();

    // Replace `eyre_hook.install()?`.
    //
    // This hook is called whenever a new eyre report is generated.
    //
    // We do not want to release the terminal here, as we also generate eyre reports when a child process is terminated.
    // In this case, a single download is set to 'failed' state, while the rest of the application keeps running.
    //
    // Instead, to print clean eyre report backtraces, we first release the terminal,
    // then propagate any error that might have been returned from the work future to the termination of `fn main()`.
    let eyre_hook = eyre_hook.into_eyre_hook();
    color_eyre::eyre::set_hook(Box::new(move |e| eyre_hook(e)))?;

    // Replace `panic_hook.install()`.
    let panic_hook = panic_hook.into_panic_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let terminal = Ui::make_terminal().expect("make terminal for panic handler");
        Ui::release_terminal(terminal).expect("release terminal for panic handler");

        panic_hook(panic_info);
    }));

    Ok(())
}
