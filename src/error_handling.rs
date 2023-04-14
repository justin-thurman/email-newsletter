/// Iterates over a chain of errors via the `source` method and prints the error with its cause
pub fn error_chain_fmt(
    error: &impl std::error::Error,
    formatter: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(formatter, "{}\n", error)?;
    let mut current = error.source();
    while let Some(cause) = current {
        writeln!(formatter, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}
