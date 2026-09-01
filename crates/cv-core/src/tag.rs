//! **`Tag` and `TagQuery`** — matching content by what it *is*, not by naming it.
//!
//! A `Tag` is a dotted label (`Enemy.Boss.Dragon`). A `TagQuery` is one of those plus an
//! **exact/inherited** toggle.
//!
//! # Why this is not a list of class references
//!
//! ⚠ **The filters-instead-of-ids problem.** A hand-maintained list of eligible classes is correct on
//! the day it is written and wrong on the day someone adds content. *"Any boss"* written as a list of
//! four bosses silently excludes the fifth; written as a query it does not. Every eligible-surface and
//! eligible-content field in the API is a `TagQuery` for this reason — and it is also why
//! [`crate::Surface`] carries tags at all.
//!
//! # Why the toggle exists rather than a default
//!
//! Both readings are legitimate and neither is safe to assume. *"Any boss"* wants inherited matching —
//! it should pick up `Enemy.Boss.Dragon`. *"This exact checkpoint class"* wants exact — inheriting
//! would quietly widen it. Picking one as a silent default makes the other into a bug that presents as
//! *content that mysteriously does or does not appear*, which is the hardest kind to trace.

use std::fmt;

/// A dotted label — `Enemy.Boss.Dragon`.
///
/// ⚠ **Segments are the unit of matching, not characters.** `Enemy.Bossy` must not match `Enemy.Boss`,
/// which a prefix test on the raw string would get wrong.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tag(String);

impl Tag {
    /// A tag from a dotted string. Empty segments are dropped rather than rejected, so `"a..b"` and
    /// `"a.b"` are the same tag.
    pub fn new(dotted: &str) -> Self {
        let cleaned: Vec<&str> = dotted.split('.').filter(|s| !s.is_empty()).collect();
        Tag(cleaned.join("."))
    }

    /// The dotted form.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The segments, outermost first.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('.').filter(|s| !s.is_empty())
    }

    /// How many segments deep this tag is. `Enemy.Boss` is 2.
    pub fn depth(&self) -> usize {
        self.segments().count()
    }

    /// Is this tag `other`, or a refinement of it?
    ///
    /// ⚠ **Segment-wise**, so `Enemy.Bossy` is *not* under `Enemy.Boss`. A `starts_with` on the
    /// string would say it was.
    pub fn is_under(&self, other: &Tag) -> bool {
        let mut theirs = other.segments();
        let mut mine = self.segments();
        loop {
            match (theirs.next(), mine.next()) {
                (None, _) => return true,
                (Some(_), None) => return false,
                (Some(t), Some(m)) if t != m => return false,
                _ => {}
            }
        }
    }

    /// The tag one level out — `Enemy.Boss.Dragon` → `Enemy.Boss`. `None` at the root.
    pub fn parent(&self) -> Option<Tag> {
        let segs: Vec<&str> = self.segments().collect();
        if segs.len() <= 1 {
            return None;
        }
        Some(Tag(segs[..segs.len() - 1].join(".")))
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A tag match, with the toggle the editor draws as a checkbox.
///
/// ⚠ **`inherited` is a field, not a default.** See the module note: assuming either reading turns the
/// other into a silent content-appearance bug.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TagQuery {
    /// What is being matched against.
    pub tag: Tag,
    /// Match refinements too (`Enemy.Boss` matches `Enemy.Boss.Dragon`), or only the tag itself.
    pub inherited: bool,
}

impl TagQuery {
    /// *"This tag and anything under it."* — the *"any boss"* reading.
    pub fn inherited(dotted: &str) -> Self {
        TagQuery {
            tag: Tag::new(dotted),
            inherited: true,
        }
    }

    /// *"This tag and nothing else."*
    pub fn exact(dotted: &str) -> Self {
        TagQuery {
            tag: Tag::new(dotted),
            inherited: false,
        }
    }

    /// Does this candidate tag match?
    pub fn matches(&self, candidate: &Tag) -> bool {
        if self.inherited {
            candidate.is_under(&self.tag)
        } else {
            candidate == &self.tag
        }
    }

    /// Does *any* of a tag set match? The usual question, since content carries several tags.
    pub fn matches_any<'a>(&self, tags: impl IntoIterator<Item = &'a Tag>) -> bool {
        tags.into_iter().any(|t| self.matches(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherited_matching_picks_up_content_that_did_not_exist_yet() {
        // ⚠ **The filters-instead-of-ids problem, stated as a test.** The query was written when only
        // `Enemy.Boss.Dragon` existed; adding `Enemy.Boss.Lich` later must not require editing it.
        let any_boss = TagQuery::inherited("Enemy.Boss");
        assert!(any_boss.matches(&Tag::new("Enemy.Boss.Dragon")));
        assert!(any_boss.matches(&Tag::new("Enemy.Boss.Lich")));
        assert!(
            any_boss.matches(&Tag::new("Enemy.Boss")),
            "and the tag itself"
        );
    }

    #[test]
    fn exact_matching_does_not_quietly_widen() {
        let this_one = TagQuery::exact("Enemy.Boss");
        assert!(this_one.matches(&Tag::new("Enemy.Boss")));
        assert!(
            !this_one.matches(&Tag::new("Enemy.Boss.Dragon")),
            "exact means exact, or the toggle would be decoration"
        );
    }

    #[test]
    fn matching_is_segment_wise_and_not_a_string_prefix() {
        // ⚠ The bug a `starts_with` implementation has and nobody notices until a tag is named
        // unluckily.
        let any_boss = TagQuery::inherited("Enemy.Boss");
        assert!(!any_boss.matches(&Tag::new("Enemy.Bossy")));
        assert!(!any_boss.matches(&Tag::new("Enemy.Bossling.Small")));
        assert!(any_boss.matches(&Tag::new("Enemy.Boss.Dragon")));
    }

    #[test]
    fn a_broader_query_never_matches_a_narrower_tag_backwards() {
        // `Enemy` covers `Enemy.Boss`; `Enemy.Boss` does not cover `Enemy`.
        assert!(TagQuery::inherited("Enemy").matches(&Tag::new("Enemy.Boss")));
        assert!(!TagQuery::inherited("Enemy.Boss").matches(&Tag::new("Enemy")));
    }

    #[test]
    fn content_carries_several_tags_and_any_of_them_may_answer() {
        let tags = [Tag::new("Prop.Light"), Tag::new("Enemy.Boss.Dragon")];
        assert!(TagQuery::inherited("Enemy.Boss").matches_any(&tags));
        assert!(!TagQuery::inherited("Surface.Water").matches_any(&tags));
    }

    #[test]
    fn tags_normalise_so_two_spellings_of_the_same_thing_are_one_tag() {
        assert_eq!(Tag::new("a..b"), Tag::new("a.b"));
        assert_eq!(Tag::new(".a.b."), Tag::new("a.b"));
        assert_eq!(Tag::new("a.b").depth(), 2);
    }

    #[test]
    fn parent_walks_out_one_level_and_stops_at_the_root() {
        let t = Tag::new("Enemy.Boss.Dragon");
        assert_eq!(t.parent(), Some(Tag::new("Enemy.Boss")));
        assert_eq!(t.parent().unwrap().parent(), Some(Tag::new("Enemy")));
        assert_eq!(Tag::new("Enemy").parent(), None);
    }
}
