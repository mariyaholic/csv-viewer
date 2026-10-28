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
pub const THREADING_MODEL: &str = "Apartment";

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
    /// A fixed descriptive value. It is removed only if it equals this string.
    Static(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryValue {
    /// Key path relative to `HKCU\Software\Classes`.
    pub key: String,
    /// `None` means the default value.
    pub name: Option<&'static str>,
    pub value: String,
    pub expected: ExpectedValue,
}

impl RegistryValue {
    /// Value used to detect a conflicting existing handler for CLSID entries.
    pub fn is_association(&self) -> bool {
        self.name.is_none() && self.expected == ExpectedValue::Clsid
    }
}

pub fn register_plan(module_path: &str) -> Vec<RegistryValue> {
    vec![
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}"),
            name: None,
            value: CLSID_DESCRIPTION.to_string(),
            expected: ExpectedValue::Static(CLSID_DESCRIPTION),
        },
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}\\InprocServer32"),
            name: None,
            value: module_path.to_string(),
            expected: ExpectedValue::ModulePath,
        },
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}\\InprocServer32"),
            name: Some("ThreadingModel"),
            value: THREADING_MODEL.to_string(),
            expected: ExpectedValue::Static(THREADING_MODEL),
        },
        RegistryValue {
            key: format!(".csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
            name: None,
            value: CLSID_CSV_PREVIEW.to_string(),
            expected: ExpectedValue::Clsid,
        },
        RegistryValue {
            key: format!("SystemFileAssociations\\.csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
            name: None,
            value: CLSID_CSV_PREVIEW.to_string(),
            expected: ExpectedValue::Clsid,
        },
    ]
}

/// Keys to remove after values are deleted. Deepest first. Parent keys that
/// may contain unrelated values (such as `.csv`) are never removed.
pub fn unregister_keys() -> Vec<String> {
    vec![
        format!("CLSID\\{CLSID_CSV_PREVIEW}\\InprocServer32"),
        format!("CLSID\\{CLSID_CSV_PREVIEW}"),
        format!(".csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
        format!(".csv\\ShellEx"),
        format!("SystemFileAssociations\\.csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
        format!("SystemFileAssociations\\.csv\\ShellEx"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_registers_the_clsid_and_csv_associations() {
        let plan = register_plan(r"C:\Program Files\CSV Viewer\csv-preview.dll");
        assert!(plan.iter().any(|entry| entry.key.ends_with("InprocServer32")
            && entry.value.ends_with("csv-preview.dll")));
        assert!(plan.iter().any(|entry| entry.name == Some("ThreadingModel")
            && entry.value == "Apartment"));
        assert_eq!(association_keys().len(), 2);
        assert!(association_keys()
            .iter()
            .all(|key| key.starts_with(".csv\\ShellEx") || key.starts_with("SystemFileAssociations")));
    }

    #[test]
    fn unregistration_is_scoped_to_our_keys() {
        let keys = unregister_keys();
        assert!(keys.iter().any(|key| key.contains(CLSID_CSV_PREVIEW)));
        assert!(!keys.iter().any(|key| key == ".csv"), "must not delete the .csv key");
        assert!(keys
            .iter()
            .all(|key| !key.starts_with("SystemFileAssociations\\.csv\\ShellEx\\{") || key.contains(PREVIEW_HANDLER_CATEGORY)));
    }

    #[test]
    fn warning_explains_the_change_and_reversal() {
        assert!(REGISTRATION_WARNING.contains("preview handler"));
        assert!(REGISTRATION_WARNING.contains("another preview handler"));
        assert!(REGISTRATION_WARNING.contains("unregistration"));
    }
}
