//! Builds a single hosts file out of several named sets.

/// One input for [`combine`]: the set's name and its raw contents.
#[derive(Debug, Clone)]
pub struct CombinedSource<'a> {
    pub name: &'a str,
    pub contents: &'a str,
}

/// Concatenates sets, prefixing each block with a `# Hosts File: <name>` marker
/// (the same marker GasMask writes into its combined files).
pub fn combine(sources: &[CombinedSource<'_>]) -> String {
    let mut out = String::new();
    for src in sources {
        out.push_str("# Hosts File: ");
        out.push_str(src.name);
        out.push('\n');
        out.push_str(src.contents.trim_end_matches('\n'));
        out.push_str("\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_with_markers() {
        let out = combine(&[
            CombinedSource { name: "A", contents: "1.1.1.1 a.test\n" },
            CombinedSource { name: "B", contents: "2.2.2.2 b.test" },
        ]);
        assert_eq!(
            out,
            "# Hosts File: A\n1.1.1.1 a.test\n\n# Hosts File: B\n2.2.2.2 b.test\n\n"
        );
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert_eq!(combine(&[]), "");
    }
}
