use wip_core::catalog::{Action, Catalog, Category, Hive, Level, Risk};
use wip_core::inventory::{AppxPackage, Inventory, NamedState, PackageState};
use wip_core::plan::{self, availability, compile, validate, Availability, Context, Op, Selection, Severity};
use wip_core::recipe::{sanitize_label, OutputSettings, Recipe};
use wip_core::WindowsFamily;

fn cat() -> Catalog {
    Catalog::builtin().expect("builtin catalog is valid")
}

fn inv() -> Inventory {
    Inventory {
        appx: vec![
            AppxPackage {
                display_name: "Clipchamp.Clipchamp".into(),
                package_name: "Clipchamp.Clipchamp_2.2.8.0_neutral_~_yxz26nhyzhsrt".into(),
            },
            AppxPackage {
                display_name: "Microsoft.BingNews".into(),
                package_name: "Microsoft.BingNews_4.1.24002.0_neutral_~_8wekyb3d8bbwe".into(),
            },
            AppxPackage {
                display_name: "Microsoft.GamingApp".into(),
                package_name: "Microsoft.GamingApp_2024.311.2341.0_neutral_~_8wekyb3d8bbwe".into(),
            },
            AppxPackage {
                display_name: "Microsoft.XboxGamingOverlay".into(),
                package_name: "Microsoft.XboxGamingOverlay_5.823.1271.0_neutral_~_8wekyb3d8bbwe".into(),
            },
        ],
        capabilities: vec![
            NamedState { name: "App.StepsRecorder~~~~0.0.1.0".into(), state: PackageState::Installed },
            NamedState { name: "Microsoft.Windows.WordPad~~~~0.0.1.0".into(), state: PackageState::NotPresent },
        ],
        features: vec![
            NamedState { name: "SMB1Protocol".into(), state: PackageState::Installed },
            NamedState { name: "NetFx3".into(), state: PackageState::Removed },
            NamedState { name: "MicrosoftWindowsPowerShellV2Root".into(), state: PackageState::Staged },
        ],
        services: ["DiagTrack", "WSearch"].into_iter().map(String::from).collect(),
        has_sxs: true,
    }
}

fn ctx<'a>(c: &'a Catalog, i: Option<&'a Inventory>) -> Context<'a> {
    Context { catalog: c, family: WindowsFamily::Win11, build: 26100, inventory: i }
}

fn sel(ids: &[&str]) -> Selection {
    ids.iter().map(|s| s.to_string()).collect()
}

#[test]
fn builtin_catalog_loads_and_covers_every_category() {
    let c = cat();
    assert!(c.options.len() > 100, "catalog has {} options", c.options.len());
    for category in Category::ALL {
        assert!(c.in_category(category).next().is_some(), "{category:?} is empty");
    }
    assert!(c.preset("light").is_some());
}

#[test]
fn every_option_is_beginner_readable() {
    for o in &cat().options {
        assert!(o.summary.len() <= 120, "{} summary too long", o.id);
        assert!(!o.undo.trim().is_empty(), "{} needs undo text", o.id);
    }
}

#[test]
fn light_preset_is_safe_for_beginners() {
    let c = cat();
    for id in &c.preset("light").unwrap().options {
        let o = c.get(id).unwrap();
        assert!(o.visible_at(Level::Beginner), "{id} not beginner-visible");
        assert!(o.risk <= Risk::Low, "{id} too risky for the light preset");
    }
}

#[test]
fn level_visibility() {
    let c = cat();
    let store = c.get("apps.store").unwrap();
    assert!(!store.visible_at(Level::Beginner));
    assert!(!store.visible_at(Level::Intermediate));
    assert!(store.visible_at(Level::Expert));
}

#[test]
fn availability_without_inventory_only_checks_version() {
    let c = cat();
    let x = ctx(&c, None);
    assert_eq!(availability(c.get("apps.clipchamp").unwrap(), &x), Availability::Available);
    assert!(matches!(availability(c.get("apps.paint3d").unwrap(), &x), Availability::NotForThisWindows(_)));
}

#[test]
fn availability_with_inventory() {
    let c = cat();
    let i = inv();
    let x = ctx(&c, Some(&i));
    assert!(availability(c.get("apps.clipchamp").unwrap(), &x).is_available());
    assert!(matches!(availability(c.get("apps.weather").unwrap(), &x), Availability::NotInImage(_)));
    assert!(availability(c.get("cap.steps_recorder").unwrap(), &x).is_available());
    assert!(matches!(availability(c.get("cap.wordpad").unwrap(), &x), Availability::AlreadyDone(_)));
    assert!(availability(c.get("feat.smb1").unwrap(), &x).is_available());
    assert!(matches!(availability(c.get("feat.powershell2").unwrap(), &x), Availability::AlreadyDone(_)));
    assert!(availability(c.get("feat.netfx3").unwrap(), &x).is_available());
    assert!(availability(c.get("svc.diagtrack").unwrap(), &x).is_available());
    assert!(matches!(availability(c.get("svc.fax").unwrap(), &x), Availability::NotInImage(_)));
}

#[test]
fn netfx3_needs_sxs() {
    let c = cat();
    let mut i = inv();
    i.has_sxs = false;
    let x = ctx(&c, Some(&i));
    assert!(matches!(availability(c.get("feat.netfx3").unwrap(), &x), Availability::NotInImage(_)));
}

#[test]
fn conflicts_are_errors_and_high_risk_warns() {
    let c = cat();
    let issues = validate(&sel(&["slim.component_cleanup", "slim.reset_base", "apps.store"]), &ctx(&c, None));
    assert!(plan::has_errors(&issues));
    assert!(issues.iter().any(|i| i.severity == Severity::Warning && i.message.contains("high risk")));
    let issues = validate(&sel(&["nope.option"]), &ctx(&c, None));
    assert!(plan::has_errors(&issues));
}

#[test]
fn compile_orders_and_resolves() {
    let c = cat();
    let i = inv();
    let s = sel(&[
        "priv.advertising_id",
        "apps.xbox",
        "feat.smb1",
        "feat.netfx3",
        "svc.diagtrack",
        "desk.file_extensions",
        "slim.component_cleanup",
        "cap.steps_recorder",
        "setup.bypass_tpm",
        "apps.weather",
    ]);
    let p = compile(&s, &ctx(&c, Some(&i))).unwrap();
    let kinds: Vec<&Op> = p.install.iter().map(|o| &o.op).collect();

    // Apps resolve to full package names, only those present.
    let appx: Vec<_> = kinds
        .iter()
        .filter_map(|o| match o {
            Op::RemoveAppx { package_name, .. } => Some(package_name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(appx.len(), 2);
    assert!(appx[0].starts_with("Microsoft.GamingApp_"));

    // Order: apps, capabilities, disable, enable, hives, cleanup last.
    let pos = |f: &dyn Fn(&Op) -> bool| kinds.iter().position(|o| f(o)).unwrap();
    let a = pos(&|o| matches!(o, Op::RemoveAppx { .. }));
    let cap = pos(&|o| matches!(o, Op::RemoveCapability { .. }));
    let dis = pos(&|o| matches!(o, Op::DisableFeature { .. }));
    let en = pos(&|o| matches!(o, Op::EnableFeature { .. }));
    let load = pos(&|o| matches!(o, Op::LoadHive { .. }));
    assert!(a < cap && cap < dis && dis < en && en < load);
    assert!(matches!(kinds.last().unwrap(), Op::ComponentCleanup { reset_base: false }));

    // Every hive load has a matching unload, in order.
    for hive in [Hive::Software, Hive::System, Hive::DefaultUser] {
        let l = kinds.iter().position(|o| **o == Op::LoadHive { hive });
        let u = kinds.iter().position(|o| **o == Op::UnloadHive { hive });
        assert_eq!(l.is_some(), u.is_some());
        if let (Some(l), Some(u)) = (l, u) {
            assert!(l < u);
            for o in &kinds[l + 1..u] {
                match o {
                    Op::RegSet { hive: h, .. } | Op::RegDelete { hive: h, .. } => assert_eq!(*h, hive),
                    _ => panic!("non-registry op inside hive group: {o:?}"),
                }
            }
        }
    }

    // Services become SYSTEM hive writes.
    assert!(kinds.iter().any(|o| matches!(o, Op::RegSet { key, .. } if key == r"ControlSet001\Services\DiagTrack")));

    // Boot.wim changes are separate.
    assert_eq!(p.boot.first().map(|o| &o.op), Some(&Op::LoadHive { hive: Hive::BootSystem }));
    assert_eq!(p.boot.last().map(|o| &o.op), Some(&Op::UnloadHive { hive: Hive::BootSystem }));
    assert!(!p.install.iter().any(|o| matches!(o.op, Op::LoadHive { hive: Hive::BootSystem })));

    // Weather isn't in the image: reported as skipped.
    assert!(p.skipped.iter().any(|(id, _)| id == "apps.weather"));
}

#[test]
fn compile_requires_inventory() {
    let c = cat();
    assert!(compile(&sel(&["apps.clipchamp"]), &ctx(&c, None)).is_err());
}

#[test]
fn compile_rejects_conflicts() {
    let c = cat();
    let i = inv();
    assert!(compile(&sel(&["slim.component_cleanup", "slim.reset_base"]), &ctx(&c, Some(&i))).is_err());
}

#[test]
fn catalog_rejects_bad_entries() {
    let c = cat();
    let mut bad = c.options.clone();
    bad[0].actions = vec![Action::RegSet {
        hive: Hive::System,
        key: r"CurrentControlSet\Services\X".into(),
        value: "Start".into(),
        data: wip_core::RegData::Dword(4),
    }];
    assert!(Catalog::from_parts(bad, vec![]).is_err());

    let mut dup = c.options.clone();
    dup.push(dup[0].clone());
    assert!(Catalog::from_parts(dup, vec![]).is_err());

    let mut risky = c.options.clone();
    let i = risky.iter().position(|o| o.risk == Risk::High).unwrap();
    risky[i].breaks.clear();
    assert!(Catalog::from_parts(risky, vec![]).is_err());
}

#[test]
fn recipe_roundtrip_and_unknowns() {
    let c = cat();
    let s = sel(&["apps.clipchamp", "priv.tips"]);
    let r = Recipe::new(&s, &OutputSettings::default());
    let json = r.to_json();
    let loaded = Recipe::parse(json.as_bytes(), &c).unwrap();
    assert_eq!(loaded.selection, s);
    assert!(loaded.unknown.is_empty());

    let with_unknown = json.replace("priv.tips", "future.option");
    let loaded = Recipe::parse(with_unknown.as_bytes(), &c).unwrap();
    assert_eq!(loaded.unknown, vec!["future.option".to_string()]);

    let future = json.replace("\"schema\": 2", "\"schema\": 99");
    assert!(Recipe::parse(future.as_bytes(), &c).is_err());
    assert!(Recipe::parse(b"not json", &c).is_err());
    assert!(Recipe::parse(&vec![b' '; 300 * 1024], &c).is_err());
}

#[test]
fn labels_are_sanitized() {
    assert_eq!(sanitize_label("Win 11 Lite!"), "WIN_11_LITE_");
    assert_eq!(sanitize_label(""), "WINISOPILOT");
    assert_eq!(sanitize_label(&"x".repeat(50)).len(), 32);
}

#[test]
fn raw_items_exclude_catalogued_and_protected() {
    use wip_core::raw;
    let c = cat();
    let mut i = inv();
    i.appx
        .push(AppxPackage { display_name: "Contoso.Thing".into(), package_name: "Contoso.Thing_1.0_x64__abc".into() });
    i.appx.push(AppxPackage {
        display_name: "Microsoft.VCLibs.140.00".into(),
        package_name: "Microsoft.VCLibs.140.00_14_x64__8we".into(),
    });
    let items = raw::items(&i, &c);
    let ids: Vec<&str> = items.iter().map(|r| r.id.as_str()).collect();
    assert!(ids.contains(&"raw.appx:Contoso.Thing"));
    assert!(!ids.iter().any(|s| s.contains("VCLibs")));
    assert!(!ids.iter().any(|s| s.contains("Clipchamp")), "catalogued apps are not duplicated");
    assert!(
        ids.contains(&"raw.featon:MicrosoftWindowsPowerShellV2Root") || ids.iter().all(|s| !s.contains("PowerShellV2"))
    );

    let s = sel(&["raw.appx:Contoso.Thing", "raw.appx:Microsoft.VCLibs.140.00"]);
    let p = compile(&s, &ctx(&c, Some(&i))).unwrap();
    assert_eq!(p.install.len(), 1);
    assert!(p.skipped.iter().any(|(id, _)| id.contains("VCLibs")));
    let acks = plan::needs_ack(&s, &c);
    assert_eq!(acks.len(), 2);
}

#[test]
fn recipe_port_detects_same_changed_missing_new() {
    use wip_core::port::{port, PortStatus};
    let c = cat();
    let i = inv();
    let s = sel(&["apps.clipchamp", "apps.xbox", "cap.steps_recorder", "feat.smb1", "priv.tips"]);
    let x = ctx(&c, Some(&i));
    let mut r = Recipe::with_fingerprint(&s, &OutputSettings::default(), &x, "Professional");
    // Pretend the recipe came from an older catalog without priv.tips's sibling options.
    r.known_options.retain(|id| id != "priv.advertising_id");
    let loaded = Recipe::parse(r.to_json().as_bytes(), &c).unwrap();
    assert_eq!(loaded.source.as_ref().unwrap().build, 26100);

    // Same image: everything Same, one new option.
    let rep = port(&loaded, &x);
    assert!(rep.items.iter().all(|it| it.status == PortStatus::Same), "{rep:?}");
    assert_eq!(rep.new_options, vec!["priv.advertising_id".to_string()]);

    // New build: Xbox overlay gone, Clipchamp gone, capability version bumped.
    let mut i2 = inv();
    i2.appx.retain(|a| a.display_name != "Microsoft.XboxGamingOverlay" && a.display_name != "Clipchamp.Clipchamp");
    i2.capabilities[0].name = "App.StepsRecorder~~~~0.0.2.0".into();
    let x2 = Context { catalog: &c, family: WindowsFamily::Win11, build: 26200, inventory: Some(&i2) };
    let rep = port(&loaded, &x2);
    let st = |id: &str| rep.items.iter().find(|it| it.id == id).unwrap().status.clone();
    assert!(matches!(st("apps.clipchamp"), PortStatus::Missing(_)));
    assert!(
        matches!(st("apps.xbox"), PortStatus::Changed { ref gone, .. } if gone.iter().any(|g| g.contains("xboxgamingoverlay")))
    );
    assert_eq!(st("cap.steps_recorder"), PortStatus::Same, "version-only change is not a change");
    assert_eq!(rep.from_build, Some(26100));
    assert_eq!(rep.to_build, 26200);
    assert!(!rep.applicable_selection().contains("apps.clipchamp"));

    // Old-format recipe: Unverified.
    let old = Recipe::parse(Recipe::new(&s, &OutputSettings::default()).to_json().as_bytes(), &c).unwrap();
    let rep = port(&old, &x);
    assert!(rep.items.iter().all(|it| it.status == PortStatus::Unverified));
}
