//! `autounattend.xml` builder for the finished media.
//!
//! Only settings from Microsoft's unattend reference are written:
//! Microsoft-Windows-International-Core(-WinPE) language settings,
//! Microsoft-Windows-Setup `UserData/AcceptEula`, and
//! Microsoft-Windows-Shell-Setup `OOBE`, `UserAccounts/LocalAccounts`,
//! `TimeZone` and `FirstLogonCommands`.
//!
//! Deliberately absent: `DiskConfiguration` (Setup always asks which disk to
//! use, so nothing is wiped without the user choosing it), product keys, and
//! `ProtectYourPC` (Express settings also cover web-content protection; the
//! user answers that page).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Answers {
    /// Write an answer file at all.
    pub enabled: bool,
    /// Language tag for Setup, Windows display, regional format, keyboard
    /// (e.g. `en-US`). Empty = Setup asks.
    pub language: String,
    /// Keyboard, as a language tag or `0409:00000409`. Empty = same as language.
    pub keyboard: String,
    /// Windows time zone ID, e.g. `Pacific Standard Time`. Empty = Setup asks.
    pub time_zone: String,
    pub accept_eula: bool,
    /// Create this local administrator instead of asking for a Microsoft
    /// account. Empty = Setup asks as usual.
    pub account_name: String,
    /// Stored in plain text in the answer file (deleted after first logon).
    /// Empty = no password. Never written to recipes.
    #[serde(skip)]
    pub account_password: String,
    pub hide_wireless_setup: bool,
}

impl Default for Answers {
    fn default() -> Self {
        Answers {
            enabled: false,
            language: String::new(),
            keyboard: String::new(),
            time_zone: String::new(),
            accept_eula: true,
            account_name: String::new(),
            account_password: String::new(),
            hide_wireless_setup: false,
        }
    }
}

/// Common Windows time zone IDs offered in the picker (any ID can be typed).
pub const TIME_ZONES: &[&str] = &[
    "UTC",
    "GMT Standard Time",
    "W. Europe Standard Time",
    "Central Europe Standard Time",
    "Romance Standard Time",
    "E. Europe Standard Time",
    "Russian Standard Time",
    "India Standard Time",
    "China Standard Time",
    "Tokyo Standard Time",
    "AUS Eastern Standard Time",
    "New Zealand Standard Time",
    "Eastern Standard Time",
    "Central Standard Time",
    "Mountain Standard Time",
    "US Mountain Standard Time",
    "Pacific Standard Time",
    "Alaskan Standard Time",
    "Hawaiian Standard Time",
    "SA Pacific Standard Time",
    "E. South America Standard Time",
];

const RESERVED_NAMES: &[&str] =
    &["administrator", "guest", "defaultaccount", "wdagutilityaccount", "system", "none", "user", "users", "admin"];

impl Answers {
    pub fn creates_account(&self) -> bool {
        !self.account_name.trim().is_empty()
    }

    /// Problems that block writing the file.
    pub fn validate(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.enabled {
            return out;
        }
        if !self.language.is_empty() && !is_language_tag(&self.language) {
            out.push(format!("\"{}\" isn't a language tag like en-US.", self.language));
        }
        let kb = self.keyboard.trim();
        if !kb.is_empty() && !is_language_tag(kb) && !is_keyboard_id(kb) {
            out.push(format!("\"{kb}\" isn't a keyboard like en-US or 0409:00000409."));
        }
        let tz = self.time_zone.trim();
        if !tz.is_empty() && !tz.chars().all(|c| c.is_ascii_alphanumeric() || " .+-()".contains(c)) {
            out.push(format!("\"{tz}\" isn't a Windows time zone ID."));
        }
        if self.creates_account() {
            let n = self.account_name.trim();
            if n.chars().count() > 20 {
                out.push("Account names can be at most 20 characters.".into());
            }
            if n.chars().any(|c| "\\/[]:;|=,+*?<>\"@".contains(c) || c.is_control()) || n.ends_with('.') {
                out.push("Account names can't contain \\ / [ ] : ; | = , + * ? < > \" @ or end with a dot.".into());
            }
            if RESERVED_NAMES.contains(&n.to_ascii_lowercase().as_str()) {
                out.push(format!("\"{n}\" is a name Windows reserves; pick another."));
            }
        } else if !self.account_password.is_empty() {
            out.push("A password needs an account name.".into());
        }
        out
    }

    /// Plain-language summary of what Setup will skip.
    pub fn summary(&self) -> Vec<String> {
        let mut s = Vec::new();
        if !self.language.is_empty() {
            s.push(format!("Language and region: {}", self.language));
        }
        if !self.time_zone.is_empty() {
            s.push(format!("Time zone: {}", self.time_zone));
        }
        if self.accept_eula {
            s.push("License terms accepted".into());
        }
        if self.creates_account() {
            let pw = if self.account_password.is_empty() { "no password" } else { "with password" };
            s.push(format!("Local account {} ({pw}), no Microsoft account screens", self.account_name.trim()));
        }
        s.push("You still choose the disk".into());
        s
    }

    /// The answer file. `arch` is the image architecture (`x64`, `ARM64`, `x86`).
    pub fn to_xml(&self, arch: &str) -> String {
        let arch = match arch.to_ascii_lowercase().as_str() {
            "arm64" => "arm64",
            "x86" => "x86",
            _ => "amd64",
        };
        let comp = |name: &str| {
            format!(
                "    <component name=\"{name}\" processorArchitecture=\"{arch}\" publicKeyToken=\"31bf3856ad364e35\" language=\"neutral\" versionScope=\"nonSxS\" xmlns:wcm=\"http://schemas.microsoft.com/WMIConfig/2002/State\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n"
            )
        };
        let lang = esc(self.language.trim());
        let kb = if self.keyboard.trim().is_empty() { lang.clone() } else { esc(self.keyboard.trim()) };
        let mut x = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!-- Made by WinISOPilot. Setup always asks which disk to use. -->\n<unattend xmlns=\"urn:schemas-microsoft-com:unattend\">\n");

        // windowsPE
        let mut pe = String::new();
        if !lang.is_empty() {
            pe += &comp("Microsoft-Windows-International-Core-WinPE");
            pe += &format!("      <SetupUILanguage><UILanguage>{lang}</UILanguage></SetupUILanguage>\n");
            pe += &format!("      <InputLocale>{kb}</InputLocale>\n      <SystemLocale>{lang}</SystemLocale>\n      <UILanguage>{lang}</UILanguage>\n      <UserLocale>{lang}</UserLocale>\n");
            pe += "    </component>\n";
        }
        if self.accept_eula {
            pe += &comp("Microsoft-Windows-Setup");
            pe += "      <UserData><AcceptEula>true</AcceptEula></UserData>\n";
            pe += "    </component>\n";
        }
        if !pe.is_empty() {
            x += "  <settings pass=\"windowsPE\">\n";
            x += &pe;
            x += "  </settings>\n";
        }

        // oobeSystem
        let mut oobe = String::new();
        if !lang.is_empty() {
            oobe += &comp("Microsoft-Windows-International-Core");
            oobe += &format!("      <InputLocale>{kb}</InputLocale>\n      <SystemLocale>{lang}</SystemLocale>\n      <UILanguage>{lang}</UILanguage>\n      <UserLocale>{lang}</UserLocale>\n");
            oobe += "    </component>\n";
        }
        let mut shell = String::new();
        let mut oobe_el = String::new();
        if self.accept_eula {
            oobe_el += "        <HideEULAPage>true</HideEULAPage>\n";
        }
        if self.creates_account() {
            oobe_el += "        <HideOnlineAccountScreens>true</HideOnlineAccountScreens>\n";
        }
        if self.hide_wireless_setup {
            oobe_el += "        <HideWirelessSetupInOOBE>true</HideWirelessSetupInOOBE>\n";
        }
        if !oobe_el.is_empty() {
            shell += &format!("      <OOBE>\n{oobe_el}      </OOBE>\n");
        }
        if self.creates_account() {
            shell += &format!(
                "      <UserAccounts>\n        <LocalAccounts>\n          <LocalAccount wcm:action=\"add\">\n            <Name>{n}</Name>\n            <DisplayName>{n}</DisplayName>\n            <Group>Administrators</Group>\n            <Password><Value>{p}</Value><PlainText>true</PlainText></Password>\n          </LocalAccount>\n        </LocalAccounts>\n      </UserAccounts>\n",
                n = esc(self.account_name.trim()),
                p = esc(&self.account_password),
            );
            // Setup keeps copies of the answer file (with the password) in
            // Panther; remove them at first logon.
            shell += "      <FirstLogonCommands>\n        <SynchronousCommand wcm:action=\"add\">\n          <Order>1</Order>\n          <CommandLine>cmd.exe /c del /f /q \"%WINDIR%\\Panther\\unattend.xml\"</CommandLine>\n          <Description>WinISOPilot: remove the stored answer file</Description>\n        </SynchronousCommand>\n      </FirstLogonCommands>\n";
        }
        if !self.time_zone.trim().is_empty() {
            shell += &format!("      <TimeZone>{}</TimeZone>\n", esc(self.time_zone.trim()));
        }
        if !shell.is_empty() {
            oobe += &comp("Microsoft-Windows-Shell-Setup");
            oobe += &shell;
            oobe += "    </component>\n";
        }
        if !oobe.is_empty() {
            x += "  <settings pass=\"oobeSystem\">\n";
            x += &oobe;
            x += "  </settings>\n";
        }
        x += "</unattend>\n";
        x
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn is_language_tag(s: &str) -> bool {
    let mut parts = s.split('-');
    let first = parts.next().unwrap_or("");
    (2..=3).contains(&first.len())
        && first.chars().all(|c| c.is_ascii_alphabetic())
        && parts.all(|p| (2..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

fn is_keyboard_id(s: &str) -> bool {
    match s.split_once(':') {
        Some((a, b)) => {
            a.len() == 4
                && b.len() == 8
                && a.chars().all(|c| c.is_ascii_hexdigit())
                && b.chars().all(|c| c.is_ascii_hexdigit())
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Answers {
        Answers {
            enabled: true,
            language: "de-DE".into(),
            keyboard: "0407:00000407".into(),
            time_zone: "W. Europe Standard Time".into(),
            accept_eula: true,
            account_name: "Sam".into(),
            account_password: "p<&>w".into(),
            hide_wireless_setup: false,
        }
    }

    #[test]
    fn validates() {
        assert!(sample().validate().is_empty());
        let mut a = sample();
        a.account_name = "Administrator".into();
        a.language = "german".into();
        a.keyboard = "0407".into();
        assert_eq!(a.validate().len(), 3);
        let mut b = sample();
        b.account_name = "a/b".into();
        assert_eq!(b.validate().len(), 1);
        let mut c = Answers { enabled: true, ..Answers::default() };
        c.account_password = "x".into();
        assert_eq!(c.validate().len(), 1);
        assert!(Answers::default().validate().is_empty());
    }

    #[test]
    fn xml_has_only_documented_settings_and_never_partitions() {
        let x = sample().to_xml("ARM64");
        assert!(x.contains("processorArchitecture=\"arm64\""));
        assert!(x.contains("<InputLocale>0407:00000407</InputLocale>"));
        assert!(x.contains("<SetupUILanguage><UILanguage>de-DE</UILanguage></SetupUILanguage>"));
        assert!(x.contains("<AcceptEula>true</AcceptEula>"));
        assert!(x.contains("<HideOnlineAccountScreens>true</HideOnlineAccountScreens>"));
        assert!(x.contains("<Value>p&lt;&amp;&gt;w</Value>"));
        assert!(x.contains("<TimeZone>W. Europe Standard Time</TimeZone>"));
        assert!(x.contains("Panther\\unattend.xml"));
        for banned in ["DiskConfiguration", "WillWipeDisk", "ProductKey", "ProtectYourPC", "BypassNRO"] {
            assert!(!x.contains(banned), "{banned}");
        }
    }

    #[test]
    fn minimal_file_when_only_eula() {
        let a = Answers { enabled: true, ..Answers::default() };
        let x = a.to_xml("x64");
        assert!(x.contains("pass=\"windowsPE\""));
        assert!(x.contains("HideEULAPage"));
        assert!(!x.contains("UserAccounts"));
        assert!(!x.contains("International-Core"));
    }
}
