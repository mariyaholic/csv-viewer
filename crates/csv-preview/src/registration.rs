// SPDX-License-Identifier: Apache-2.0

//! Pure registration plan for the Explorer preview handler.
//!
//! Registration is per-user (`HKCU\Software\Classes`) and opt-in. The plan is
//! kept separate from Win32 calls so key layout and conflict policy can be
//! tested on Linux. The installer/UI must show [`REGISTRATION_WARNING`] before
//! changing any association; unregistration removes only values that still
//! match this handler.

pub const CLSID_CSV_PREVIEW: &str = "{8f6a0f5e-3c4b-4a2e-9d7f-5b1c2e3a4f60}";
pub const PREVIEW_HANDLER_CATEGORY: &str = "{8895b1c6-b41f-4c1c-a562-0d564250836f}";
pub const CLSID_DESCRIPTION: &str = "CSV viewer and preview";
pub const PREVHOST_APPID: &str = "{6d2b5079-2f0b-48dd-ab7f-97cec514d30b}";
pub const PREVIEW_HANDLERS_KEY: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\PreviewHandlers";
pub const THREADING_MODEL: &str = "Apartment";

/// `DisableLowILProcessIsolation` (REG_DWORD, 1): opt this preview handler out
/// of the Low integrity level that `prevhost.exe` otherwise imposes. Without
/// it the spool copy under `%TEMP%` fails before any window appears. Microsoft
/// documents this per-handler opt-out (the PowerToys spelling).
pub const DISABLE_LOWIL_VALUE_NAME: &str = "DisableLowILProcessIsolation";
pub const DISABLE_LOWIL_VALUE: u32 = 1;

pub const REGISTRATION_WARNING: &str =
    "This will make CSV viewer and preview the preview handler for .csv files for the current \
     Windows user. It may replace another preview handler. You can unregister it at any time; \
     unregistration only removes values that still point to this handler.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedValue {
    /// The value must match the handler CLSID before removal.
    Clsid,
    /// The value must match the installed module path before removal.
    ModulePath,
    /// A fixed `REG_SZ` string. It is removed only if it equals this string.
    Static(&'static str),
    /// A fixed `REG_DWORD` number. It is removed only if it equals this number.
    Dword(u32),
}

/// A registry value written by the plan and read back for ownership checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A `REG_SZ` string.
    Text(String),
    /// A `REG_DWORD` integer.
    Dword(u32),
}

impl Value {
    pub fn text(value: impl Into<String>) -> Self {
        Value::Text(value.into())
    }

    /// The string form of a text value, or `None` for a DWORD.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(value) => Some(value),
            Value::Dword(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryValue {
    /// Key path relative to `HKCU\Software\Classes`, except PreviewHandlers.
    pub key: String,
    /// `None` means the default value.
    pub name: Option<&'static str>,
    pub value: Value,
    pub expected: ExpectedValue,
}

impl RegistryValue {
    /// Value used to detect a conflicting existing handler for CLSID entries.
    pub fn is_association(&self) -> bool {
        self.name.is_none() && self.expected == ExpectedValue::Clsid
    }

    /// Whether an existing `REG_SZ`/`REG_DWORD` value still belongs to this
    /// handler and may therefore be removed on unregistration.
    pub fn still_owned_by_handler(&self, existing: &Value) -> bool {
        match (&self.expected, existing) {
            (ExpectedValue::Clsid, Value::Text(text)) => {
                text.eq_ignore_ascii_case(CLSID_CSV_PREVIEW)
            }
            (ExpectedValue::ModulePath, Value::Text(text)) => {
                text.eq_ignore_ascii_case(self.value.as_text().unwrap_or(""))
            }
            (ExpectedValue::Static(expected), Value::Text(text)) => text == expected,
            (ExpectedValue::Dword(expected), Value::Dword(found)) => expected == found,
            _ => false,
        }
    }
}

pub fn register_plan(module_path: &str) -> Vec<RegistryValue> {
    // Microsoft, "How to register a preview handler": AppID selects the
    // 64-bit prevhost surrogate and PreviewHandlers names the handler.
    vec![
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}"),
            name: Some("AppID"),
            value: Value::text(PREVHOST_APPID),
            expected: ExpectedValue::Static(PREVHOST_APPID),
        },
        // Low integrity isolation would deny the spool copy under %TEMP%; the
        // documented per-handler opt-out keeps the preview working.
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}"),
            name: Some(DISABLE_LOWIL_VALUE_NAME),
            value: Value::Dword(DISABLE_LOWIL_VALUE),
            expected: ExpectedValue::Dword(DISABLE_LOWIL_VALUE),
        },
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}"),
            name: Some("DisplayName"),
            value: Value::text(CLSID_DESCRIPTION),
            expected: ExpectedValue::Static(CLSID_DESCRIPTION),
        },
        RegistryValue {
            key: PREVIEW_HANDLERS_KEY.to_string(),
            name: Some(CLSID_CSV_PREVIEW),
            value: Value::text(CLSID_DESCRIPTION),
            expected: ExpectedValue::Static(CLSID_DESCRIPTION),
        },
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}"),
            name: None,
            value: Value::text(CLSID_DESCRIPTION),
            expected: ExpectedValue::Static(CLSID_DESCRIPTION),
        },
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}\\InprocServer32"),
            name: None,
            value: Value::text(module_path),
            expected: ExpectedValue::ModulePath,
        },
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}\\InprocServer32"),
            name: Some("ThreadingModel"),
            value: Value::text(THREADING_MODEL),
            expected: ExpectedValue::Static(THREADING_MODEL),
        },
        RegistryValue {
            key: format!(".csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
            name: None,
            value: Value::text(CLSID_CSV_PREVIEW),
            expected: ExpectedValue::Clsid,
        },
        RegistryValue {
            key: format!("SystemFileAssociations\\.csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
            name: None,
            value: Value::text(CLSID_CSV_PREVIEW),
            expected: ExpectedValue::Clsid,
        },
    ]
}

/// Keys that must be inspected for an existing preview handler before an
/// opt-in registration is allowed to proceed.
pub fn association_keys() -> Vec<String> {
    register_plan("")
        .into_iter()
        .filter(RegistryValue::is_association)
        .map(|entry| entry.key)
        .collect()
}

/// Registry operations needed by unregistration. Missing keys and values are
/// represented as `None` or success; other failures must be returned.
pub trait Registry {
    type Error;
    /// The per-user view (`HKCU\Software\Classes`).
    fn read(&mut self, key: &str, name: Option<&str>) -> Result<Option<Value>, Self::Error>;
    /// The merged `HKEY_CLASSES_ROOT` view the conflict preflight reads. A
    /// registry that only stores per-user values (the fakes) may fall back to
    /// [`Registry::read`]; on real Windows this is the HKLM∪HKCU merge.
    fn read_classes_root(
        &mut self,
        key: &str,
        name: Option<&str>,
    ) -> Result<Option<Value>, Self::Error> {
        self.read(key, name)
    }
    fn delete_value(&mut self, key: &str, name: Option<&str>) -> Result<(), Self::Error>;
    fn write(&mut self, entry: &RegistryValue) -> Result<(), Self::Error>;
}

/// Restore prior values after a failed write, unless another writer changed them.
/// Rollback errors are returned alongside the original failure.
pub fn apply_plan<R: Registry>(
    registry: &mut R,
    plan: &[RegistryValue],
) -> Result<(), (R::Error, Vec<R::Error>)> {
    let mut written: Vec<(&RegistryValue, Option<Value>)> = Vec::new();
    for entry in plan {
        let previous = match registry.read(&entry.key, entry.name) {
            Ok(value) => value,
            Err(error) => return Err((error, rollback(registry, written))),
        };
        if let Err(error) = registry.write(entry) {
            return Err((error, rollback(registry, written)));
        }
        written.push((entry, previous));
    }
    Ok(())
}

fn rollback<R: Registry>(
    registry: &mut R,
    written: Vec<(&RegistryValue, Option<Value>)>,
) -> Vec<R::Error> {
    let mut errors = Vec::new();
    for (entry, previous) in written.into_iter().rev() {
        match registry.read(&entry.key, entry.name) {
            Ok(Some(current)) if current == entry.value => {
                let result = match previous {
                    Some(value) => {
                        let mut old = entry.clone();
                        old.value = value;
                        registry.write(&old)
                    }
                    None => registry.delete_value(&entry.key, entry.name),
                };
                if let Err(error) = result {
                    errors.push(error);
                }
            }
            Ok(_) => {}
            Err(error) => errors.push(error),
        }
    }
    errors
}

pub fn unregister<R: Registry>(registry: &mut R, module_path: &str) -> Result<(), R::Error> {
    for entry in register_plan(module_path) {
        if let Some(existing) = registry.read(&entry.key, entry.name)? {
            if entry.still_owned_by_handler(&existing) {
                registry.delete_value(&entry.key, entry.name)?;
            }
        }
    }
    // Shared keys must remain: checking emptiness and deleting is not atomic.
    // The exclusive CLSID tree could be deleted, but is also left in place.
    Ok(())
}

/// A foreign preview handler that occupies an association this registration
/// would take over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociationConflict {
    /// Classes-root key path of the conflicted association.
    pub key: String,
    /// The foreign CLSID currently registered there.
    pub foreign_clsid: String,
    /// `true` when the conflict is already in the user hive; `false` for a
    /// machine-wide handler that only the merged classes root sees.
    pub in_user_hive: bool,
}

impl AssociationConflict {
    /// One visible refusal line.
    pub fn message(&self) -> String {
        if self.in_user_hive {
            format!(
                "{:?} currently belongs to another preview handler ({}); \
                 either keep it or register with force",
                self.key, self.foreign_clsid
            )
        } else {
            format!(
                "a machine-wide preview handler ({}) currently covers {:?}; \
                 either keep it or register with force",
                self.foreign_clsid, self.key
            )
        }
    }
}

/// Preflight for a registration: every association in the plan that a
/// foreign handler occupies, in the user hive or machine-wide through the
/// merged classes root. `force` is a caller decision on top of this result.
pub fn find_foreign_associations<R: Registry>(
    registry: &mut R,
    module_path: &str,
) -> Result<Vec<AssociationConflict>, R::Error> {
    let foreign = |existing: Option<Value>| match existing {
        Some(Value::Text(text))
            if !text.is_empty() && !text.eq_ignore_ascii_case(CLSID_CSV_PREVIEW) =>
        {
            Some(text)
        }
        _ => None,
    };
    let mut conflicts = Vec::new();
    for entry in register_plan(module_path) {
        if !entry.is_association() {
            continue;
        }
        let user_foreign = foreign(registry.read(&entry.key, entry.name)?);
        let merged_foreign = foreign(registry.read_classes_root(&entry.key, entry.name)?);
        match (user_foreign, merged_foreign) {
            (Some(existing), _) => conflicts.push(AssociationConflict {
                key: entry.key,
                foreign_clsid: existing,
                in_user_hive: true,
            }),
            (None, Some(existing)) => conflicts.push(AssociationConflict {
                key: entry.key,
                foreign_clsid: existing,
                in_user_hive: false,
            }),
            (None, None) => {}
        }
    }
    Ok(conflicts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_registers_the_clsid_and_csv_associations() {
        let plan = register_plan(r"C:\Program Files\CSV Viewer\csv-preview.dll");
        assert!(plan
            .iter()
            .any(|entry| entry.key.ends_with("InprocServer32")
                && entry
                    .value
                    .as_text()
                    .is_some_and(|value| value.ends_with("csv-preview.dll"))));
        assert!(plan
            .iter()
            .any(|entry| entry.name == Some("ThreadingModel")
                && entry.value == Value::text("Apartment")));
        assert!(
            plan.iter()
                .any(|entry| entry.name == Some("AppID")
                    && entry.value == Value::text(PREVHOST_APPID))
        );
        assert!(plan.iter().any(
            |entry| entry.key == PREVIEW_HANDLERS_KEY && entry.name == Some(CLSID_CSV_PREVIEW)
        ));
        assert!(plan.iter().any(|entry| entry.name == Some("DisplayName")
            && entry.value == Value::text(CLSID_DESCRIPTION)));
        assert!(plan
            .iter()
            .any(|entry| entry.name == Some(DISABLE_LOWIL_VALUE_NAME)
                && entry.value == Value::Dword(DISABLE_LOWIL_VALUE)));
        assert_eq!(association_keys().len(), 2);
        assert!(association_keys().iter().all(
            |key| key.starts_with(".csv\\ShellEx") || key.starts_with("SystemFileAssociations")
        ));
    }

    #[test]
    fn warning_explains_the_change_and_reversal() {
        assert!(REGISTRATION_WARNING.contains("preview handler"));
        assert!(REGISTRATION_WARNING.contains("another preview handler"));
        assert!(REGISTRATION_WARNING.contains("unregistration"));
    }
    #[test]
    fn unregister_preserves_shared_keys_and_foreign_content() {
        use std::collections::BTreeMap;
        #[derive(Default)]
        struct Fake(BTreeMap<String, BTreeMap<String, Value>>);
        impl Fake {
            fn key(&mut self, path: &str) {
                let mut current = String::new();
                for part in path.split('\\') {
                    if !current.is_empty() {
                        current.push('\\');
                    }
                    current.push_str(part);
                    self.0.entry(current.clone()).or_default();
                }
            }
        }
        impl Registry for Fake {
            type Error = ();
            fn read(&mut self, key: &str, name: Option<&str>) -> Result<Option<Value>, ()> {
                Ok(self
                    .0
                    .get(key)
                    .and_then(|v| v.get(name.unwrap_or("")))
                    .cloned())
            }
            fn delete_value(&mut self, key: &str, name: Option<&str>) -> Result<(), ()> {
                if let Some(values) = self.0.get_mut(key) {
                    values.remove(name.unwrap_or(""));
                }
                Ok(())
            }
            fn write(&mut self, entry: &RegistryValue) -> Result<(), ()> {
                self.key(&entry.key);
                self.0
                    .get_mut(&entry.key)
                    .unwrap()
                    .insert(entry.name.unwrap_or("").into(), entry.value.clone());
                Ok(())
            }
        }
        let mut registry = Fake::default();
        registry.key(".csv");
        registry
            .0
            .get_mut(".csv")
            .unwrap()
            .insert("".into(), Value::text("foreign default"));
        registry.key("SystemFileAssociations\\.csv\\ShellEx\\Foreign");
        registry.key(PREVIEW_HANDLERS_KEY);
        registry
            .0
            .get_mut(PREVIEW_HANDLERS_KEY)
            .unwrap()
            .insert("Foreign".into(), Value::text("keep"));
        apply_plan(&mut registry, &register_plan("module.dll")).unwrap();
        let association =
            format!("SystemFileAssociations\\.csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}");
        registry
            .0
            .get_mut(&association)
            .unwrap()
            .insert("".into(), Value::text("foreign handler"));
        let clsid = format!("CLSID\\{CLSID_CSV_PREVIEW}");
        registry
            .0
            .get_mut(&clsid)
            .unwrap()
            .insert("Foreign".into(), Value::text("keep"));
        unregister(&mut registry, "module.dll").unwrap();
        let csv_association = format!(".csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}");
        assert!(registry.0[&csv_association].is_empty());
        assert!(registry.0.contains_key(".csv\\ShellEx"));
        assert_eq!(registry.0[".csv"][""], Value::text("foreign default"));
        assert!(registry
            .0
            .contains_key("SystemFileAssociations\\.csv\\ShellEx\\Foreign"));
        assert_eq!(registry.0[&association][""], Value::text("foreign handler"));
        assert!(registry.0[&format!("{clsid}\\InprocServer32")].is_empty());
        assert_eq!(registry.0[&clsid]["Foreign"], Value::text("keep"));
        assert_eq!(
            registry.0[PREVIEW_HANDLERS_KEY]["Foreign"],
            Value::text("keep")
        );
        assert!(!registry.0[PREVIEW_HANDLERS_KEY].contains_key(CLSID_CSV_PREVIEW));
        assert!(!registry.0[&clsid].contains_key("AppID"));
        assert!(!registry.0[&clsid].contains_key(DISABLE_LOWIL_VALUE_NAME));
    }
    #[test]
    fn failed_registration_restores_prior_values() {
        use std::collections::BTreeMap;
        #[derive(Default)]
        struct Fake {
            values: BTreeMap<String, Value>,
            writes: usize,
            keys: std::collections::BTreeSet<String>,
        }
        impl Registry for Fake {
            type Error = &'static str;
            fn read(&mut self, k: &str, _: Option<&str>) -> Result<Option<Value>, Self::Error> {
                Ok(self.values.get(k).cloned())
            }
            fn write(&mut self, e: &RegistryValue) -> Result<(), Self::Error> {
                self.writes += 1;
                if self.writes == 3 {
                    return Err("failed");
                }
                self.keys.insert(e.key.clone());
                self.values.insert(e.key.clone(), e.value.clone());
                Ok(())
            }
            fn delete_value(&mut self, k: &str, _: Option<&str>) -> Result<(), Self::Error> {
                self.values.remove(k);
                Ok(())
            }
        }
        let mut fake = Fake::default();
        let plan: Vec<_> = register_plan("module")
            .into_iter()
            .enumerate()
            .take(3)
            .map(|(i, mut e)| {
                e.key = format!(".csv\\ShellEx\\{i}");
                e
            })
            .collect();
        fake.values
            .insert(".csv\\ShellEx\\0".into(), Value::text("previous"));
        assert_eq!(apply_plan(&mut fake, &plan), Err(("failed", vec![])));
        assert_eq!(
            fake.values.get(".csv\\ShellEx\\0"),
            Some(&Value::text("previous"))
        );
        assert!(!fake.values.contains_key(".csv\\ShellEx\\1"));
        assert!(fake.keys.contains(".csv\\ShellEx\\1"));
    }

    #[test]
    fn register_plan_writes_disable_lowil_dword_and_unregister_removes_it() {
        use std::collections::BTreeMap;
        // Typed fake: stores REG_SZ/REG_DWORD distinctly so the test can prove
        // the Low-IL opt-out is written as a DWORD and removed only when owned.
        #[derive(Default)]
        struct Fake(BTreeMap<(String, String), Value>);
        impl Fake {
            fn slot(key: &str, name: &str) -> (String, String) {
                (key.to_string(), name.to_string())
            }
        }
        impl Registry for Fake {
            type Error = ();
            fn read(&mut self, key: &str, name: Option<&str>) -> Result<Option<Value>, ()> {
                Ok(self.0.get(&Fake::slot(key, name.unwrap_or(""))).cloned())
            }
            fn delete_value(&mut self, key: &str, name: Option<&str>) -> Result<(), ()> {
                self.0.remove(&Fake::slot(key, name.unwrap_or("")));
                Ok(())
            }
            fn write(&mut self, entry: &RegistryValue) -> Result<(), ()> {
                self.0.insert(
                    Fake::slot(&entry.key, entry.name.unwrap_or("")),
                    entry.value.clone(),
                );
                Ok(())
            }
        }
        let mut registry = Fake::default();
        apply_plan(&mut registry, &register_plan("module.dll")).unwrap();
        let clsid = format!("CLSID\\{CLSID_CSV_PREVIEW}");
        let slot = Fake::slot(&clsid, DISABLE_LOWIL_VALUE_NAME);
        assert_eq!(
            registry.0.get(&slot),
            Some(&Value::Dword(DISABLE_LOWIL_VALUE)),
            "registration must write the Low-IL opt-out as a DWORD"
        );
        // A foreign value under the same name is not ours: leave it in place.
        registry
            .0
            .insert(slot.clone(), Value::Dword(DISABLE_LOWIL_VALUE + 1));
        unregister(&mut registry, "module.dll").unwrap();
        assert_eq!(
            registry.0.get(&slot),
            Some(&Value::Dword(DISABLE_LOWIL_VALUE + 1)),
            "unregistration must not remove a value this handler did not write"
        );
        // Our own value is removed.
        registry
            .0
            .insert(slot.clone(), Value::Dword(DISABLE_LOWIL_VALUE));
        unregister(&mut registry, "module.dll").unwrap();
        assert!(!registry.0.contains_key(&slot));
    }

    #[test]
    fn register_refuses_when_hklm_shellex_exists() {
        use std::collections::BTreeMap;
        // The fake keeps the user hive and the machine-wide classes-root
        // mirror distinct, so a foreign handler that only HKCR sees is a
        // proof of the real conflict condition.
        #[derive(Default)]
        struct Fake {
            user: BTreeMap<String, Value>,
            root: BTreeMap<String, Value>,
        }
        impl Registry for Fake {
            type Error = ();
            fn read(&mut self, key: &str, _: Option<&str>) -> Result<Option<Value>, ()> {
                Ok(self.user.get(key).cloned())
            }
            fn read_classes_root(
                &mut self,
                key: &str,
                _: Option<&str>,
            ) -> Result<Option<Value>, ()> {
                Ok(self.root.get(key).cloned())
            }
            fn delete_value(&mut self, _: &str, _: Option<&str>) -> Result<(), ()> {
                Ok(())
            }
            fn write(&mut self, _: &RegistryValue) -> Result<(), ()> {
                Ok(())
            }
        }
        const FOREIGN: &str = "{11111111-2222-3333-4444-555555555555}";

        // Machine-only handler (HKLM style): HKCU is clean, HKCR shows it.
        let mut registry = Fake::default();
        registry.root.insert(
            format!(".csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
            Value::text(FOREIGN),
        );
        let conflicts = find_foreign_associations(&mut registry, "module.dll").unwrap();
        assert_eq!(
            conflicts,
            vec![AssociationConflict {
                key: format!(".csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
                foreign_clsid: FOREIGN.to_string(),
                in_user_hive: false,
            }]
        );
        assert!(
            conflicts[0].message().contains("machine-wide"),
            "{}",
            conflicts[0].message()
        );

        // A foreign handler already in the user hive is also reported.
        registry.user.insert(
            format!("SystemFileAssociations\\.csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
            Value::text(FOREIGN),
        );
        let conflicts = find_foreign_associations(&mut registry, "module.dll").unwrap();
        assert_eq!(conflicts.len(), 2);
        assert!(
            conflicts.iter().any(|conflict| conflict.in_user_hive
                && conflict.key
                    == format!(
                        "SystemFileAssociations\\.csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"
                    )),
            "{conflicts:?}"
        );

        // A clean registry has no conflicts, and our own CLSID is expected,
        // never foreign.
        let mut registry = Fake::default();
        for key in [
            format!(".csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
            format!("SystemFileAssociations\\.csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
        ] {
            registry.user.insert(key, Value::text(CLSID_CSV_PREVIEW));
        }
        assert!(find_foreign_associations(&mut registry, "module.dll")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn unregister_propagates_registry_errors() {
        struct Failing;
        impl Registry for Failing {
            type Error = &'static str;
            fn read(&mut self, _: &str, _: Option<&str>) -> Result<Option<Value>, Self::Error> {
                Err("read failed")
            }
            fn delete_value(&mut self, _: &str, _: Option<&str>) -> Result<(), Self::Error> {
                unreachable!()
            }
            fn write(&mut self, _: &RegistryValue) -> Result<(), Self::Error> {
                unreachable!()
            }
        }
        assert_eq!(unregister(&mut Failing, "module.dll"), Err("read failed"));
    }
}
