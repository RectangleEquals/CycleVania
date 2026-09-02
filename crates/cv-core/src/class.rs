//! **`Kind<T>`, `Ref<T>`, and the class default** — the reference machinery every other part of the
//! API leans on.
//!
//! # The bug this exists to make unrepresentable
//!
//! ⚠ **Constructing an instance to mean a kind.** An instance standing in for a type is the failure
//! that caused the visual-authoring pivot: it looks like it works, it carries the right fields, and it
//! is a *different object* every time anyone asks — so two mentions of "the same thing" silently are
//! not. `Kind<T>` names a class and constructs nothing; [`Kind::defaults`] reads that class's authored
//! values off **one core-owned object**, the same one, forever.
//!
//! ⚠ **The lattice does not use any of this.** Unlocks are table rows, settled at M03a — they trade in
//! neither instances *nor* classes. `Kind<T>` survives for the many places that genuinely name a class:
//! `CheckpointComponent.restores`, `BlocksTraversalComponent.matching`, `Interaction` targets.
//!
//! # Why `Kind<T>` and `Ref<T>` are distinct types rather than one with a flag
//!
//! They answer questions that have no overlap — *"which class"* against *"which one of them"* — and the
//! whole point of the pin type is that **choosing wrongly was never on the menu**. A single type with a
//! discriminant would put both on the menu and report the mistake afterwards, which is the design this
//! one replaced.
//!
//! Here, `Kind<T>` and `Ref<T>` cannot be interchanged at compile time, and [`PinType`] refuses the
//! same wiring as *data* — because the editor's pins are data, not Rust types.

use crate::object::ObjectId;
use crate::path::ClassPath;
use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

/// A tier-1 class that can bound a [`Kind`] or a [`Ref`].
///
/// ⚠ **Only `/Core/…` classes implement this**, because only they have a name the compiler can see.
/// A content class is bounded by whichever core class it extends, which is exactly the bound the
/// picker filters on.
///
/// ⚠ **The markers are named `…Bound`, not `…Class`.** The design already owns `ItemClass` — it is the
/// enum of item *classifications* (`PROGRESSION` · `USEFUL` · `BONUS` · `FILLER`), and a marker of the
/// same name would have been two unrelated concepts under one identifier. `Bound` also says what these
/// are: the `T` a `Kind<T>` is bounded at.
pub trait CoreClass {
    /// The mount-pointed path of this class.
    const PATH: &'static str;

    /// That path, parsed.
    fn class_path() -> ClassPath {
        ClassPath::core(Self::PATH)
    }
}

/// Declare a tier-1 class marker.
macro_rules! core_class {
    ($(#[$m:meta])* $name:ident => $path:literal) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name;
        impl CoreClass for $name {
            const PATH: &'static str = $path;
        }
    };
}

core_class!(
    /// The root of everything with identity.
    ObjectBound => "/Core/Object"
);
core_class!(
    /// Placeable, authoritative, non-behavioural.
    ActorBound => "/Core/Actor"
);
core_class!(
    /// An obtainable Actor.
    ItemBound => "/Core/Item"
);
core_class!(
    /// Attachable behaviour.
    ComponentBound => "/Core/Component"
);
core_class!(
    /// What a piece of geometry means to a mechanic.
    SurfaceBound => "/Core/Surface"
);
core_class!(
    /// A spatial delta that becomes a directed edge.
    TraversalComponentBound => "/Core/TraversalComponent"
);
core_class!(
    /// An external-asset target.
    ResourceBound => "/Core/Resource"
);
core_class!(
    /// A named, retunable limit — see [`crate::budget`].
    BudgetBound => "/Core/Budget"
);

/// What a class **is**, as registered.
///
/// ⚠ `extends` is `None` for exactly one class — `/Core/Object`. Everything else has a parent, which
/// is what makes ancestry a walk rather than a search.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassRecord {
    /// Its mount-pointed path.
    pub path: ClassPath,
    /// The class it extends.
    pub extends: Option<ClassPath>,
    /// **The class default**: authored field values, read and never built by content.
    ///
    /// ⚠ Stored as name→value pairs rather than a typed struct because a content class's fields are
    /// authored, not compiled — the core cannot have a Rust type for `/Content/Items/Hookshot`.
    pub fields: BTreeMap<String, FieldValue>,
}

/// One authored field value on a class default.
///
/// ⚠ **Deliberately a small closed set.** A class default holds *authored constants*, not computed
/// results — anything that needs a `ctx` is a hook, and a hook cannot be read without running it,
/// which is the thing `defaults()` exists to avoid.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    /// A number.
    Number(f64),
    /// A yes or no.
    Bool(bool),
    /// Text.
    Text(String),
    /// A class reference — `Kind'…'`.
    Class(ClassPath),
    /// A file reference — `Asset'…'`.
    Asset(crate::path::AssetPath),
    /// A tag.
    Tag(crate::tag::Tag),
}

impl FieldValue {
    /// Read it as a number, or `None` if it is something else.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            FieldValue::Number(v) => Some(*v),
            _ => None,
        }
    }

    /// Read it as text.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            FieldValue::Text(v) => Some(v),
            _ => None,
        }
    }

    /// Read it as a class reference.
    pub fn as_class(&self) -> Option<&ClassPath> {
        match self {
            FieldValue::Class(v) => Some(v),
            _ => None,
        }
    }
}

/// What can go wrong while registering a class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClassError {
    /// That path is already registered.
    Duplicate { path: ClassPath },
    /// It extends something that is not registered.
    UnknownParent { path: ClassPath, parent: ClassPath },
    /// A `/Core` class tried to extend a `/Content` class.
    MountViolation { path: ClassPath, parent: ClassPath },
    /// A pick fell outside the bound its pin declared.
    NotUnderBound { path: ClassPath, base: ClassPath },
    /// Nothing is registered under that path.
    Unknown { path: ClassPath },
    /// A second root was declared. There is exactly one.
    SecondRoot { path: ClassPath },
}

impl fmt::Display for ClassError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClassError::Duplicate { path } => write!(f, "{path} is already registered"),
            ClassError::UnknownParent { path, parent } => {
                write!(f, "{path} extends {parent}, which is not registered")
            }
            ClassError::MountViolation { path, parent } => write!(
                f,
                "{path} may not extend {parent} — /Core never extends /Content"
            ),
            ClassError::NotUnderBound { path, base } => {
                write!(
                    f,
                    "{path} is not under {base} — the picker never offered it"
                )
            }
            ClassError::Unknown { path } => write!(f, "{path} is not registered"),
            ClassError::SecondRoot { path } => write!(
                f,
                "{path} declares no parent, but /Core/Object is already the root"
            ),
        }
    }
}

impl std::error::Error for ClassError {}

/// Every registered class, and the one class-default object each of them owns.
///
/// ⚠ **`defaults()` reads from here and nothing writes to it during generation.** That is the property
/// that makes a class default safe to hand out: it is authored once at load, shared by every reader,
/// and a mechanic that mutated one would be changing what a *class* means halfway through a solve.
#[derive(Clone, Debug, Default)]
pub struct ClassRegistry {
    classes: BTreeMap<ClassPath, ClassRecord>,
}

impl ClassRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        ClassRegistry::default()
    }

    /// **The tier-1 tree**, rooted at `/Core/Object` — [`05-object-model`'s tree][tree], registered.
    ///
    /// ⚠ **The whole tree, not a starter set.** A project that had to hand-register `/Core/Budget`
    /// before it could reference one would be re-declaring the core in every project, and two projects
    /// would eventually disagree about what `/Core/Component` extends.
    ///
    /// [tree]: https://example.invalid
    pub fn with_core() -> Self {
        let mut r = ClassRegistry::new();
        r.register_root(ObjectBound::class_path())
            .expect("the root registers into an empty registry");
        // Parents come before children, which is also the order `register` requires.
        for (path, parent) in [
            // The two behavioural halves.
            ("/Core/Actor", "/Core/Object"),
            ("/Core/Item", "/Core/Actor"),
            ("/Core/Component", "/Core/Object"),
            ("/Core/MeshComponent", "/Core/Component"),
            ("/Core/ShapeComponent", "/Core/Component"),
            ("/Core/MountComponent", "/Core/Component"),
            ("/Core/TraversalComponent", "/Core/Component"),
            ("/Core/CheckpointComponent", "/Core/Component"),
            ("/Core/FastTravelComponent", "/Core/Component"),
            ("/Core/StateSetterComponent", "/Core/Component"),
            ("/Core/BlocksTraversalComponent", "/Core/Component"),
            // Geometry and meaning.
            ("/Core/Surface", "/Core/Object"),
            ("/Core/Shape", "/Core/Object"),
            ("/Core/CollisionBody", "/Core/Object"),
            // The only external-asset targets.
            ("/Core/Resource", "/Core/Object"),
            ("/Core/MeshResource", "/Core/Resource"),
            ("/Core/CurveTableResource", "/Core/Resource"),
            ("/Core/UnlockTableResource", "/Core/Resource"),
            // Answers and obligations.
            ("/Core/Rule", "/Core/Object"),
            ("/Core/Verdict", "/Core/Object"),
            ("/Core/Interaction", "/Core/Object"),
            ("/Core/Route", "/Core/Object"),
            ("/Core/Budget", "/Core/Object"),
            ("/Core/Path", "/Core/Object"),
            ("/Core/PlacementNeed", "/Core/Object"),
            ("/Core/Constraint", "/Core/Object"),
            ("/Core/Preference", "/Core/Object"),
            ("/Core/ScheduleRule", "/Core/Object"),
            ("/Core/Rationale", "/Core/Object"),
        ] {
            r.register(ClassPath::core(path), ClassPath::core(parent))
                .expect("the tier-1 tree is well-formed");
        }
        r
    }

    /// Register the single root — the only class with no parent.
    pub fn register_root(&mut self, path: ClassPath) -> Result<(), ClassError> {
        if let Some(existing) = self.classes.values().find(|c| c.extends.is_none()) {
            if existing.path != path {
                return Err(ClassError::SecondRoot { path });
            }
        }
        self.insert(ClassRecord {
            path,
            extends: None,
            fields: BTreeMap::new(),
        })
    }

    /// Register a class that extends another.
    pub fn register(&mut self, path: ClassPath, extends: ClassPath) -> Result<(), ClassError> {
        if !self.classes.contains_key(&extends) {
            return Err(ClassError::UnknownParent {
                path,
                parent: extends,
            });
        }
        if !path.mount().may_extend(extends.mount()) {
            return Err(ClassError::MountViolation {
                path,
                parent: extends,
            });
        }
        self.insert(ClassRecord {
            path,
            extends: Some(extends),
            fields: BTreeMap::new(),
        })
    }

    fn insert(&mut self, record: ClassRecord) -> Result<(), ClassError> {
        if self.classes.contains_key(&record.path) {
            return Err(ClassError::Duplicate { path: record.path });
        }
        self.classes.insert(record.path.clone(), record);
        Ok(())
    }

    /// Author a field on a class's default.
    ///
    /// ⚠ **An authoring operation, not a runtime one.** Called while loading schematics; calling it
    /// during a solve would change what a class means partway through one.
    pub fn author(
        &mut self,
        path: &ClassPath,
        field: &str,
        value: FieldValue,
    ) -> Result<(), ClassError> {
        let record = self
            .classes
            .get_mut(path)
            .ok_or_else(|| ClassError::Unknown { path: path.clone() })?;
        record.fields.insert(field.to_string(), value);
        Ok(())
    }

    /// The record for a path.
    pub fn get(&self, path: &ClassPath) -> Option<&ClassRecord> {
        self.classes.get(path)
    }

    /// Is this path registered?
    pub fn contains(&self, path: &ClassPath) -> bool {
        self.classes.contains_key(path)
    }

    /// How many classes are registered.
    pub fn len(&self) -> usize {
        self.classes.len()
    }

    /// Is the registry empty?
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }

    /// Every registered path, in path order.
    pub fn paths(&self) -> impl Iterator<Item = &ClassPath> {
        self.classes.keys()
    }

    /// The ancestry chain, self first, root last.
    ///
    /// ⚠ **Self is included**, because *"is a `/Core/Actor`"* must be true of `/Core/Actor`. Excluding
    /// it would make `is_a` mean *strictly derived from*, which is not what any call site wants.
    ///
    /// Returns an empty chain for an unregistered path rather than panicking — an unregistered class
    /// is a load-time diagnostic, not a reason to abort a query.
    pub fn ancestry(&self, path: &ClassPath) -> Vec<&ClassPath> {
        let mut out = Vec::new();
        let mut cursor = self.classes.get(path);
        // Bounded by the registry size: a cycle cannot be registered, but a bound costs nothing and
        // turns a would-be hang into a truncated answer.
        for _ in 0..=self.classes.len() {
            let Some(record) = cursor else { break };
            out.push(&record.path);
            let Some(parent) = &record.extends else { break };
            cursor = self.classes.get(parent);
        }
        out
    }

    /// Is `path` `base`, or derived from it?
    pub fn is_a(&self, path: &ClassPath, base: &ClassPath) -> bool {
        self.ancestry(path).into_iter().any(|p| p == base)
    }

    /// Read one field off a class's default, **inheriting from ancestors**.
    ///
    /// ⚠ **Inheritance here is the whole feature.** A subclass that overrides one field must not have
    /// to restate the other twenty; a lookup that stopped at the class itself would force exactly
    /// that, and every schematic would drift out of sync with its parent one forgotten field at a
    /// time.
    pub fn field(&self, path: &ClassPath, name: &str) -> Option<&FieldValue> {
        self.ancestry(path)
            .into_iter()
            .find_map(|p| self.classes.get(p)?.fields.get(name))
    }

    /// Every class registered under `base`, self included, in path order.
    ///
    /// ⚠ **This is the `Kind<T>` picker's list.** Choosing wrongly is not an error a developer is told
    /// about afterwards — it was never on the menu.
    pub fn subclasses_of(&self, base: &ClassPath) -> Vec<&ClassPath> {
        self.classes.keys().filter(|p| self.is_a(p, base)).collect()
    }
}

/// A picked **class** — nothing is constructed.
///
/// ⚠ `T` bounds the picker: a `Kind<ActorBound>` may only hold a path that `is_a` `/Core/Actor`.
#[derive(Debug)]
pub struct Kind<T: CoreClass> {
    path: ClassPath,
    marker: PhantomData<fn() -> T>,
}

// Derived impls would demand `T: Clone + PartialEq`, which is wrong: `T` is a marker that no value of
// this type ever holds.
impl<T: CoreClass> Clone for Kind<T> {
    fn clone(&self) -> Self {
        Kind {
            path: self.path.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: CoreClass> PartialEq for Kind<T> {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl<T: CoreClass> Eq for Kind<T> {}

impl<T: CoreClass> Kind<T> {
    /// Pick a class, checked against the bound.
    ///
    /// ⚠ Fails when the path is unregistered or is not under `T` — the two ways a picker could have
    /// offered something it should not have.
    pub fn new(registry: &ClassRegistry, path: ClassPath) -> Result<Self, ClassError> {
        let base = T::class_path();
        if !registry.contains(&path) {
            return Err(ClassError::Unknown { path });
        }
        if !registry.is_a(&path, &base) {
            return Err(ClassError::NotUnderBound { path, base });
        }
        Ok(Kind {
            path,
            marker: PhantomData,
        })
    }

    /// The bound itself, which is always a legal pick.
    pub fn base() -> ClassPath {
        T::class_path()
    }

    /// The picked path.
    pub fn path(&self) -> &ClassPath {
        &self.path
    }

    /// Is the picked class `other`, or derived from it?
    pub fn is_a(&self, registry: &ClassRegistry, other: &ClassPath) -> bool {
        registry.is_a(&self.path, other)
    }

    /// **The class default** — one core-owned object per class, read and never built.
    ///
    /// ⚠ **This is how a class's authored values are read without instantiating anything**, and the
    /// distinction that keeps it from being the old bug is that content never calls a constructor.
    /// The returned reference is stable: asking twice gives the same id.
    pub fn defaults(&self) -> Ref<T> {
        Ref {
            id: class_default_id(&self.path),
            marker: PhantomData,
        }
    }

    /// Read one authored field off the class default, inheriting from ancestors.
    pub fn default_field<'r>(
        &self,
        registry: &'r ClassRegistry,
        name: &str,
    ) -> Option<&'r FieldValue> {
        registry.field(&self.path, name)
    }

    /// Widen the bound — a `Kind<ItemBound>` is also a `Kind<ActorBound>`.
    ///
    /// ⚠ Only ever widens, and the type system enforces the direction: narrowing needs the registry
    /// to check, which is what [`Kind::new`] is for.
    pub fn upcast<U: CoreClass>(&self, registry: &ClassRegistry) -> Option<Kind<U>> {
        Kind::<U>::new(registry, self.path.clone()).ok()
    }
}

impl<T: CoreClass> fmt::Display for Kind<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Kind'{}'", self.path)
    }
}

/// A live **instance** reference.
///
/// ⚠ **Cannot be confused with a [`Kind`]** — not by a developer, because the pins render
/// differently and hold different things, and not by the compiler, because these are two types.
#[derive(Debug)]
pub struct Ref<T: CoreClass> {
    id: ObjectId,
    marker: PhantomData<fn() -> T>,
}

impl<T: CoreClass> Clone for Ref<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: CoreClass> Copy for Ref<T> {}

impl<T: CoreClass> PartialEq for Ref<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: CoreClass> Eq for Ref<T> {}

impl<T: CoreClass> Ref<T> {
    /// Refer to an existing instance.
    pub fn new(id: ObjectId) -> Self {
        Ref {
            id,
            marker: PhantomData,
        }
    }

    /// The instance's id.
    pub fn id(self) -> ObjectId {
        self.id
    }
}

impl<T: CoreClass> fmt::Display for Ref<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ref'{}'#{}", T::PATH, self.id)
    }
}

/// The id of a class's one default object.
///
/// ⚠ **Derived from the path**, so it is the same id in every process, every run, and every build —
/// which is what makes *"one core-owned object per class"* true rather than merely intended.
pub fn class_default_id(path: &ClassPath) -> ObjectId {
    ObjectId::derived("class_default", path.as_str())
}

/// A pin's declared type, as **data**.
///
/// ⚠ **The editor's pins are data, not Rust types.** A CVB file says `Type=Kind'/Core/Component'`, and
/// something has to refuse the wire from a `Ref'…'` pin at load time — before any Rust type exists to
/// have prevented it. This is that check. Parsing the notation is [M11]'s; the rule is this
/// milestone's, because the rule is what the green criterion is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PinType {
    /// A class picker bounded at this path.
    Kind(ClassPath),
    /// An instance of this class.
    Ref(ClassPath),
    /// A resource class — pair it with an `Asset'…'` value.
    Resource(ClassPath),
    /// A tag.
    Tag,
    /// A tag match with an exact/inherited toggle.
    TagQuery,
    /// A plain value — a number, a bool, a string, a struct.
    Value(&'static str),
}

impl PinType {
    /// May a value of type `source` be wired into a pin of this type?
    ///
    /// ⚠ **Kind and Ref never connect, in either direction**, and it is not a subtyping question that
    /// happens to fail — they are different questions. *"Which class"* has no reading as *"which one
    /// of them"*, so a connection between them is meaningless rather than merely unsafe.
    ///
    /// Within a family, a narrower class flows into a wider pin: a `Kind'/Core/Item'` fits a
    /// `Kind'/Core/Actor'` pin, because every Item is an Actor.
    pub fn accepts(&self, source: &PinType, registry: &ClassRegistry) -> bool {
        match (self, source) {
            (PinType::Kind(base), PinType::Kind(picked))
            | (PinType::Ref(base), PinType::Ref(picked))
            | (PinType::Resource(base), PinType::Resource(picked)) => registry.is_a(picked, base),
            (PinType::Tag, PinType::Tag) => true,
            (PinType::TagQuery, PinType::TagQuery) => true,
            (PinType::Value(a), PinType::Value(b)) => a == b,
            _ => false,
        }
    }

    /// The notation this renders as in a `.cvs` file.
    pub fn notation(&self) -> String {
        match self {
            PinType::Kind(p) => format!("Kind'{p}'"),
            PinType::Ref(p) => format!("Ref'{p}'"),
            PinType::Resource(p) => format!("Resource'{p}'"),
            PinType::Tag => "Tag".to_string(),
            PinType::TagQuery => "TagQuery".to_string(),
            PinType::Value(name) => (*name).to_string(),
        }
    }
}

impl fmt::Display for PinType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.notation())
    }
}

/// A resource reference: **two facts carried in two places** — the class, and the file.
///
/// ⚠ **Never one fact.** The path alone says nothing about how to read the bytes; the core resolves it
/// with *that class's own loader* rather than guessing a format from an extension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceRef {
    /// The resource **class** — a type position.
    pub class: ClassPath,
    /// The **file** — a value position.
    pub asset: crate::path::AssetPath,
}

impl ResourceRef {
    /// Pair a resource class with a file.
    pub fn new(class: ClassPath, asset: crate::path::AssetPath) -> Self {
        ResourceRef { class, asset }
    }

    /// Is the class actually a resource class?
    pub fn is_well_formed(&self, registry: &ClassRegistry) -> bool {
        registry.is_a(&self.class, &ResourceBound::class_path())
    }
}

impl fmt::Display for ResourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Resource'{}' Value=Asset'{}'", self.class, self.asset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::AssetPath;

    fn content(p: &str) -> ClassPath {
        ClassPath::new(p).unwrap()
    }

    /// A registry with the tier-1 tree plus one authored schematic that extends `/Core/Item`.
    fn project() -> ClassRegistry {
        let mut r = ClassRegistry::with_core();
        r.register(
            content("/Content/Items/Hookshot"),
            ClassPath::core("/Core/Item"),
        )
        .unwrap();
        r.register(
            content("/Content/Items/Longshot"),
            content("/Content/Items/Hookshot"),
        )
        .unwrap();
        r.author(
            &content("/Content/Items/Hookshot"),
            "range",
            FieldValue::Number(30.0),
        )
        .unwrap();
        r.author(
            &content("/Content/Items/Hookshot"),
            "tether_kind",
            FieldValue::Text("rope".into()),
        )
        .unwrap();
        r.author(
            &content("/Content/Items/Longshot"),
            "range",
            FieldValue::Number(60.0),
        )
        .unwrap();
        r
    }

    // --- the green criterion -----------------------------------------------------------------

    #[test]
    fn defaults_reads_an_authored_field_with_nothing_instantiated() {
        // ⚠ **The milestone's green criterion.** Reading a class's authored values must not require
        // constructing one — constructing to mean a kind is the bug that caused the pivot.
        let r = project();
        let hookshot = Kind::<ItemBound>::new(&r, content("/Content/Items/Hookshot")).unwrap();

        assert_eq!(
            hookshot
                .default_field(&r, "range")
                .and_then(FieldValue::as_number),
            Some(30.0)
        );
        assert_eq!(
            hookshot
                .default_field(&r, "tether_kind")
                .and_then(FieldValue::as_text),
            Some("rope")
        );
    }

    #[test]
    fn there_is_exactly_one_default_object_per_class_and_it_never_moves() {
        // ⚠ *"One core-owned object per class"* is only true if the id is derived rather than
        // allocated. An allocated id would differ between processes, and two readers of "the same
        // class default" would silently disagree.
        let r = project();
        let a = Kind::<ItemBound>::new(&r, content("/Content/Items/Hookshot")).unwrap();
        let b = Kind::<ItemBound>::new(&r, content("/Content/Items/Hookshot")).unwrap();
        assert_eq!(a.defaults(), b.defaults());
        assert_eq!(a.defaults().id(), class_default_id(a.path()));

        let other = Kind::<ItemBound>::new(&r, content("/Content/Items/Longshot")).unwrap();
        assert_ne!(
            a.defaults(),
            other.defaults(),
            "a different class, a different default"
        );
    }

    #[test]
    fn a_kind_pin_refuses_a_ref_and_a_ref_pin_refuses_a_kind() {
        // ⚠ **The other half of the criterion.** Not a subtyping failure — *"which class"* has no
        // reading as *"which one of them"*, so the connection is meaningless in both directions.
        let r = project();
        let actor_kind = PinType::Kind(ClassPath::core("/Core/Actor"));
        let actor_ref = PinType::Ref(ClassPath::core("/Core/Actor"));

        assert!(!actor_kind.accepts(&actor_ref, &r));
        assert!(!actor_ref.accepts(&actor_kind, &r));
        assert!(
            actor_kind.accepts(&actor_kind, &r),
            "the same pin type still connects to itself"
        );
    }

    // --- the picker --------------------------------------------------------------------------

    #[test]
    fn the_picker_lists_only_subclasses_so_a_wrong_pick_is_never_on_the_menu() {
        let r = project();
        let items = r.subclasses_of(&ClassPath::core("/Core/Item"));
        let names: Vec<&str> = items.iter().map(|p| p.as_str()).collect();
        assert!(
            names.contains(&"/Core/Item"),
            "the bound itself is a legal pick"
        );
        assert!(names.contains(&"/Content/Items/Hookshot"));
        assert!(names.contains(&"/Content/Items/Longshot"));
        assert!(!names.contains(&"/Core/Component"));
        assert!(!names.contains(&"/Core/Actor"), "an Actor is not an Item");
    }

    #[test]
    fn picking_outside_the_bound_is_refused_rather_than_reported_later() {
        let r = project();
        let wrong = Kind::<ItemBound>::new(&r, ClassPath::core("/Core/Component"));
        assert!(matches!(wrong, Err(ClassError::NotUnderBound { .. })));

        let missing = Kind::<ItemBound>::new(&r, content("/Content/Items/Grapple"));
        assert!(matches!(missing, Err(ClassError::Unknown { .. })));
    }

    #[test]
    fn a_narrower_kind_widens_and_a_wider_one_does_not_narrow() {
        let r = project();
        let hookshot = Kind::<ItemBound>::new(&r, content("/Content/Items/Hookshot")).unwrap();
        let as_actor = hookshot.upcast::<ActorBound>(&r);
        assert!(as_actor.is_some(), "every Item is an Actor");

        let plain_actor = Kind::<ActorBound>::new(&r, ClassPath::core("/Core/Actor")).unwrap();
        assert!(
            plain_actor.upcast::<ItemBound>(&r).is_none(),
            "not every Actor is an Item"
        );
    }

    #[test]
    fn a_narrower_class_flows_into_a_wider_pin_within_one_family() {
        let r = project();
        let actor_pin = PinType::Kind(ClassPath::core("/Core/Actor"));
        let item_value = PinType::Kind(content("/Content/Items/Hookshot"));
        assert!(
            actor_pin.accepts(&item_value, &r),
            "every Hookshot is an Actor"
        );

        let item_pin = PinType::Kind(ClassPath::core("/Core/Item"));
        let component_value = PinType::Kind(ClassPath::core("/Core/Component"));
        assert!(!item_pin.accepts(&component_value, &r));
    }

    // --- inheritance -------------------------------------------------------------------------

    #[test]
    fn a_subclass_inherits_the_fields_it_did_not_restate() {
        // ⚠ Without this a subclass overriding one field would have to restate the other twenty, and
        // schematics would drift out of sync with their parents one forgotten field at a time.
        let r = project();
        let longshot = Kind::<ItemBound>::new(&r, content("/Content/Items/Longshot")).unwrap();
        assert_eq!(
            longshot
                .default_field(&r, "range")
                .and_then(FieldValue::as_number),
            Some(60.0),
            "the override wins"
        );
        assert_eq!(
            longshot
                .default_field(&r, "tether_kind")
                .and_then(FieldValue::as_text),
            Some("rope"),
            "and the rest is inherited"
        );
    }

    #[test]
    fn ancestry_includes_self_because_is_a_must_be_true_of_itself() {
        let r = project();
        let p = content("/Content/Items/Longshot");
        let chain: Vec<&str> = r.ancestry(&p).iter().map(|c| c.as_str()).collect();
        assert_eq!(
            chain,
            vec![
                "/Content/Items/Longshot",
                "/Content/Items/Hookshot",
                "/Core/Item",
                "/Core/Actor",
                "/Core/Object"
            ]
        );
        assert!(r.is_a(&p, &p), "a class is itself");
    }

    #[test]
    fn an_unregistered_path_answers_rather_than_panicking() {
        // A load-time diagnostic, not a reason to abort a query.
        let r = project();
        let ghost = content("/Content/Items/Ghost");
        assert!(r.ancestry(&ghost).is_empty());
        assert!(!r.is_a(&ghost, &ClassPath::core("/Core/Object")));
        assert_eq!(r.field(&ghost, "range"), None);
    }

    // --- the registry's own rules ---------------------------------------------------------------

    #[test]
    fn core_may_not_extend_content() {
        // ⚠ Otherwise a project could move the tier-1 surface under itself, and every guarantee about
        // `/Core/…` would become a guarantee about whatever the project last edited.
        let mut r = project();
        let bad = r.register(
            ClassPath::core("/Core/SneakyThing"),
            content("/Content/Items/Hookshot"),
        );
        assert!(matches!(bad, Err(ClassError::MountViolation { .. })));
    }

    #[test]
    fn a_cycle_in_ancestry_is_unrepresentable_rather_than_detected() {
        // ⚠ **Stronger than a cycle check.** `register` demands the parent already exist, so building
        // `A extends B extends A` would require registering each before the other. There is no order
        // that does it, which is why there is no `Cycle` error to report.
        let mut r = ClassRegistry::with_core();
        assert!(matches!(
            r.register(content("/Content/A"), content("/Content/B")),
            Err(ClassError::UnknownParent { .. })
        ));
        r.register(content("/Content/B"), ClassPath::core("/Core/Actor"))
            .unwrap();
        r.register(content("/Content/A"), content("/Content/B"))
            .unwrap();
        // Closing the loop needs a *second* registration of B, which is refused outright.
        assert!(matches!(
            r.register(content("/Content/B"), content("/Content/A")),
            Err(ClassError::Duplicate { .. })
        ));
        // And ancestry still terminates.
        assert_eq!(r.ancestry(&content("/Content/A")).len(), 4);
    }

    #[test]
    fn there_is_exactly_one_root() {
        let mut r = ClassRegistry::with_core();
        let second = r.register_root(content("/Content/MyOwnRoot"));
        assert!(matches!(second, Err(ClassError::SecondRoot { .. })));
    }

    #[test]
    fn a_class_cannot_extend_something_unregistered_or_be_registered_twice() {
        let mut r = ClassRegistry::with_core();
        assert!(matches!(
            r.register(content("/Content/A"), content("/Content/Nope")),
            Err(ClassError::UnknownParent { .. })
        ));
        r.register(content("/Content/A"), ClassPath::core("/Core/Actor"))
            .unwrap();
        assert!(matches!(
            r.register(content("/Content/A"), ClassPath::core("/Core/Actor")),
            Err(ClassError::Duplicate { .. })
        ));
    }

    #[test]
    fn authoring_onto_an_unregistered_class_is_refused() {
        let mut r = project();
        assert!(matches!(
            r.author(&content("/Content/Nope"), "x", FieldValue::Bool(true)),
            Err(ClassError::Unknown { .. })
        ));
    }

    // --- resources ---------------------------------------------------------------------------

    #[test]
    fn a_resource_reference_is_two_facts_and_neither_stands_alone() {
        // ⚠ The class is a type position, the path is a value position. The core loads the file with
        // *that class's* loader rather than guessing a format from the extension.
        let r = project();
        let mesh = ResourceRef::new(
            ClassPath::core("/Core/MeshResource"),
            AssetPath::new("/Content/Meshes/hookshot.glb").unwrap(),
        );
        assert!(mesh.is_well_formed(&r));

        let nonsense = ResourceRef::new(
            ClassPath::core("/Core/Actor"),
            AssetPath::new("/Content/Meshes/hookshot.glb").unwrap(),
        );
        assert!(
            !nonsense.is_well_formed(&r),
            "an Actor is not a resource class"
        );
    }

    #[test]
    fn a_resource_pin_takes_neither_a_kind_nor_a_ref() {
        let r = project();
        let res = PinType::Resource(ClassPath::core("/Core/Resource"));
        assert!(!res.accepts(&PinType::Kind(ClassPath::core("/Core/Resource")), &r));
        assert!(!res.accepts(&PinType::Ref(ClassPath::core("/Core/Resource")), &r));
        assert!(res.accepts(&PinType::Resource(ClassPath::core("/Core/Resource")), &r));
    }

    #[test]
    fn pin_types_render_as_the_notation_a_file_carries() {
        assert_eq!(
            PinType::Kind(ClassPath::core("/Core/Component")).notation(),
            "Kind'/Core/Component'"
        );
        assert_eq!(
            PinType::Resource(ClassPath::core("/Core/Resource")).notation(),
            "Resource'/Core/Resource'"
        );
        assert_eq!(PinType::TagQuery.notation(), "TagQuery");
    }

    #[test]
    fn a_value_pin_only_takes_the_same_value_type() {
        let r = project();
        assert!(PinType::Value("float").accepts(&PinType::Value("float"), &r));
        assert!(!PinType::Value("float").accepts(&PinType::Value("Vec3"), &r));
        assert!(!PinType::Value("float").accepts(&PinType::Tag, &r));
    }

    #[test]
    fn a_kind_and_a_ref_of_the_same_class_are_different_values() {
        // The compile-time half: these are two types, so the confusion is unrepresentable rather than
        // merely reported. This test exists to record that they carry different payloads at all.
        let r = project();
        let k = Kind::<ItemBound>::new(&r, content("/Content/Items/Hookshot")).unwrap();
        let instance: Ref<ItemBound> = Ref::new(ObjectId::derived("actor", "hookshot_01"));
        assert_ne!(k.defaults().id(), instance.id());
        assert!(k.to_string().starts_with("Kind'"));
        assert!(instance.to_string().starts_with("Ref'"));
    }
}
