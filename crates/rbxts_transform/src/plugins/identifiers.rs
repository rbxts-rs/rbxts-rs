//! The names a seam name avoids: the file's identifier set as TypeScript 7.1 keeps it, tsrs's
//! `SourceFile::has_identifier` (tsgo's `SourceFile.Identifiers`, the set its printer checks a unique name against:
//! `isFileLevelUniqueName`). It holds the text of every identifier, private identifier, and string, numeric,
//! big-integer and no-substitution template literal in the tree, wherever it sits (a numeric literal as its value's
//! text, `0x10` as `16`); JSDoc is not in the tree. Internal to the seam's naming (`PluginFileState::name_taken`,
//! `PluginFileState::seam_hint`); plugins reach it only through `Cx::is_usable_local_name` and the names the seam
//! picks.

use tsrs_ast::{Kind, Node, SourceFile};
use tsrs_core::P;

/// The texts in the file's identifier set that start with `_`: what a seam temporary's rendered name (`_hint`,
/// `_hint_<n>`) can meet. `PluginFileState::seam_hint` has to list them, and tsrs answers only whether a text is in
/// the set; this is its walk (`collect_identifiers_for_source_file`), kept to those texts.
pub(crate) fn underscore_identifiers(file: P<SourceFile>) -> Vec<&'static str> {
    fn walk(node: P<Node>, out: &mut Vec<&'static str>) -> bool {
        if matches!(
            node.kind(),
            Kind::Identifier
                | Kind::PrivateIdentifier
                | Kind::StringLiteral
                | Kind::NumericLiteral
                | Kind::BigIntLiteral
                | Kind::NoSubstitutionTemplateLiteral
        ) {
            let text = node.text();
            if text.starts_with('_') && !out.contains(&text) {
                out.push(text);
            }
        }
        node.for_each_child(&mut |child| walk(child, out));
        false
    }
    let mut out = Vec::new();
    walk(file.as_node(), &mut out);
    debug_assert!(out.iter().all(|name| file.has_identifier(name)), "a text tsrs's identifier set lacks");
    out
}
