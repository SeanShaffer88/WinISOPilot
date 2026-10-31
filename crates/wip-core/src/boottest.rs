//! Boot test: install the built ISO in a throwaway Hyper-V VM and check,
//! from inside the installed Windows, that every change actually landed.
//!
//! This module generates the answer file and the in-guest check script from
//! the compiled plan, and scores the results. The engine runs the VM.

use crate::catalog::{Hive, RegData};
use crate::plan::{CompiledPlan, Op};
use serde::{Deserialize, Serialize};

/// Microsoft's published generic (KMS client setup) keys. They only select
/// the edition during setup; they don't activate anything.
/// Source: learn.microsoft.com/windows-server/get-started/kms-client-activation-keys
pub fn setup_key(edition_id: &str) -> Option<&'static str> {
    let k = match edition_id.to_ascii_lowercase().as_str() {
        "professional" => "W269N-WFGWX-YVC9B-4J6C9-T83GX",
        "professionaln" => "MH37W-N47XK-V7XM9-C7227-GCQG9",
        "professionalworkstation" => "NRG8B-VKK3Q-CXVCJ-9G2XF-6Q84J",
        "professionalworkstationn" => "9FNHH-K3HBT-3W4TD-6383H-6XYWF",
        "professionaleducation" => "6TP4R-GNPTD-KYYHQ-7B7DP-J447Y",
        "professionaleducationn" => "YVWGF-BXNMC-HTQYQ-CPQ99-66QFC",
        "education" => "NW6C2-QMPVW-D7KKK-3GKT6-VCFB2",
        "educationn" => "2WH4N-8QGBV-H22JP-CT43Q-MDWWJ",
        "enterprise" => "NPPR9-FWDCX-D2C8J-H872K-2YT43",
        "enterprisen" => "DPH2V-TTNVB-4X9Q3-TJR4H-KHJW4",
        _ => return None,
    };
    Some(k)
}

/// Single-quoted PowerShell literal.
pub fn ps_lit(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestTarget {
    /// e.g. "en-US"
    pub language: String,
    /// Image index inside the built ISO.
    pub index: u32,
    pub edition_id: String,
    /// "amd64" or "arm64"
    pub arch: String,
}

impl TestTarget {
    pub fn arch_from(dism_arch: &str) -> String {
        if dism_arch.eq_ignore_ascii_case("ARM64") {
            "arm64".into()
        } else {
            "amd64".into()
        }
    }
}

/// PowerShell that the test user's first logon runs: finds the check
/// script on the answer disc and runs it. Passed with `-EncodedCommand`.
pub const LOADER: &str =
    "$f = Get-PSDrive -PSProvider FileSystem | ForEach-Object { Join-Path $_.Root 'wip-verify.ps1' } | \
Where-Object { Test-Path $_ } | Select-Object -First 1; if ($f) { Invoke-Expression (Get-Content -Raw $f) }";

/// UTF-16LE base64, as `powershell -EncodedCommand` expects.
pub fn encode_command(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    base64(&bytes)
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Answer file for an unattended install on the VM's empty disk 0 (UEFI/GPT),
/// a local test account with automatic first logon, and the check script.
pub fn unattend_xml(t: &TestTarget) -> String {
    let lang = xml_escape(&t.language);
    let arch = xml_escape(&t.arch);
    let key = match setup_key(&t.edition_id) {
        Some(k) => format!("<ProductKey><Key>{k}</Key><WillShowUI>OnError</WillShowUI></ProductKey>"),
        None => String::new(),
    };
    let cmd = format!("powershell.exe -NoProfile -EncodedCommand {}", encode_command(LOADER));
    let comp = |name: &str| {
        format!(
            "<component name=\"{name}\" processorArchitecture=\"{arch}\" publicKeyToken=\"31bf3856ad364e35\" language=\"neutral\" versionScope=\"nonSxS\" xmlns:wcm=\"http://schemas.microsoft.com/WMIConfig/2002/State\">"
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<unattend xmlns="urn:schemas-microsoft-com:unattend">
  <settings pass="windowsPE">
    {intl_pe}
      <SetupUILanguage><UILanguage>{lang}</UILanguage></SetupUILanguage>
      <InputLocale>{lang}</InputLocale><SystemLocale>{lang}</SystemLocale><UILanguage>{lang}</UILanguage><UserLocale>{lang}</UserLocale>
    </component>
    {setup}
      <DiskConfiguration>
        <Disk wcm:action="add">
          <DiskID>0</DiskID>
          <WillWipeDisk>true</WillWipeDisk>
          <CreatePartitions>
            <CreatePartition wcm:action="add"><Order>1</Order><Type>EFI</Type><Size>300</Size></CreatePartition>
            <CreatePartition wcm:action="add"><Order>2</Order><Type>MSR</Type><Size>16</Size></CreatePartition>
            <CreatePartition wcm:action="add"><Order>3</Order><Type>Primary</Type><Extend>true</Extend></CreatePartition>
          </CreatePartitions>
          <ModifyPartitions>
            <ModifyPartition wcm:action="add"><Order>1</Order><PartitionID>1</PartitionID><Format>FAT32</Format><Label>System</Label></ModifyPartition>
            <ModifyPartition wcm:action="add"><Order>2</Order><PartitionID>2</PartitionID></ModifyPartition>
            <ModifyPartition wcm:action="add"><Order>3</Order><PartitionID>3</PartitionID><Format>NTFS</Format><Label>Windows</Label><Letter>C</Letter></ModifyPartition>
          </ModifyPartitions>
        </Disk>
      </DiskConfiguration>
      <ImageInstall>
        <OSImage>
          <InstallFrom><MetaData wcm:action="add"><Key>/IMAGE/INDEX</Key><Value>{index}</Value></MetaData></InstallFrom>
          <InstallTo><DiskID>0</DiskID><PartitionID>3</PartitionID></InstallTo>
        </OSImage>
      </ImageInstall>
      <UserData><AcceptEula>true</AcceptEula>{key}</UserData>
    </component>
  </settings>
  <settings pass="specialize">
    {shell}
      <ComputerName>WIPTEST</ComputerName>
    </component>
  </settings>
  <settings pass="oobeSystem">
    {intl}
      <InputLocale>{lang}</InputLocale><SystemLocale>{lang}</SystemLocale><UILanguage>{lang}</UILanguage><UserLocale>{lang}</UserLocale>
    </component>
    {shell}
      <OOBE>
        <HideEULAPage>true</HideEULAPage>
        <HideOEMRegistrationScreen>true</HideOEMRegistrationScreen>
        <HideOnlineAccountScreens>true</HideOnlineAccountScreens>
        <HideWirelessSetupInOOBE>true</HideWirelessSetupInOOBE>
        <ProtectYourPC>3</ProtectYourPC>
      </OOBE>
      <UserAccounts>
        <LocalAccounts>
          <LocalAccount wcm:action="add">
            <Name>wiptest</Name>
            <Group>Administrators</Group>
            <Password><Value></Value><PlainText>true</PlainText></Password>
          </LocalAccount>
        </LocalAccounts>
      </UserAccounts>
      <AutoLogon>
        <Enabled>true</Enabled>
        <Username>wiptest</Username>
        <LogonCount>1</LogonCount>
        <Password><Value></Value><PlainText>true</PlainText></Password>
      </AutoLogon>
      <FirstLogonCommands>
        <SynchronousCommand wcm:action="add">
          <Order>1</Order>
          <CommandLine>{cmd}</CommandLine>
          <Description>WinISOPilot boot test checks</Description>
        </SynchronousCommand>
      </FirstLogonCommands>
    </component>
  </settings>
</unattend>
"#,
        intl_pe = comp("Microsoft-Windows-International-Core-WinPE"),
        setup = comp("Microsoft-Windows-Setup"),
        shell = comp("Microsoft-Windows-Shell-Setup"),
        intl = comp("Microsoft-Windows-International-Core"),
        index = t.index,
    )
}

/// One thing to check inside the installed Windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub id: String,
    /// Option that caused it.
    pub origin: String,
    pub what: String,
    /// PowerShell expression that sets `$ok` (bool) and `$detail` (string).
    #[serde(skip)]
    pub script: String,
}

fn live_key(hive: Hive, key: &str) -> Option<String> {
    match hive {
        Hive::Software => Some(format!(r"HKEY_LOCAL_MACHINE\SOFTWARE\{key}")),
        Hive::System => {
            let k = key.strip_prefix(r"ControlSet001\").map(|r| format!(r"CurrentControlSet\{r}"));
            Some(format!(r"HKEY_LOCAL_MACHINE\SYSTEM\{}", k.as_deref().unwrap_or(key)))
        }
        Hive::DefaultUser => Some(format!(r"HKEY_CURRENT_USER\{key}")),
        Hive::BootSystem => None,
    }
}

fn value_name(v: &str) -> String {
    if v.is_empty() {
        "(default)".into()
    } else {
        v.to_string()
    }
}

/// `KB5043080` from a Microsoft update file name, if it has one.
fn kb_number(path: &str) -> Option<String> {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit(['\\', '/']).next()?;
    let i = name.find("kb")?;
    let digits: String = name[i + 2..].chars().take_while(|c| c.is_ascii_digit()).collect();
    (digits.len() >= 6).then(|| format!("KB{digits}"))
}

/// Builds the list of checks from what the build actually did.
pub fn checks(plan: &CompiledPlan) -> Vec<Check> {
    let mut out = Vec::new();
    for p in &plan.install {
        let origin = p.origin.clone();
        let c = match &p.op {
            Op::RemoveAppx { display_name, .. } => Some(Check {
                id: format!("app:{display_name}"),
                origin,
                what: format!("App {display_name} is not installed"),
                script: format!(
                    "$n = {n}; $hit = ($prov -contains $n) -or ($user -contains $n); $ok = -not $hit; $detail = $(if ($hit) {{ 'still installed' }} else {{ '' }})",
                    n = ps_lit(display_name)
                ),
            }),
            Op::RemoveCapability { name } => Some(Check {
                id: format!("cap:{name}"),
                origin,
                what: format!("Capability {name} is removed"),
                script: format!(
                    "$c = Get-WindowsCapability -Online -Name {n}; $ok = ($c.State -ne 'Installed'); $detail = [string]$c.State",
                    n = ps_lit(name)
                ),
            }),
            Op::DisableFeature { name, .. } | Op::EnableFeature { name, .. } => {
                let want_on = matches!(p.op, Op::EnableFeature { .. });
                Some(Check {
                    id: format!("feat:{name}"),
                    origin,
                    what: format!("Feature {name} is {}", if want_on { "on" } else { "off" }),
                    script: format!(
                        "$f = Get-WindowsOptionalFeature -Online -FeatureName {n}; $ok = (($f.State -eq 'Enabled') -eq ${want}); $detail = [string]$f.State",
                        n = ps_lit(name),
                        want = if want_on { "true" } else { "false" }
                    ),
                })
            }
            Op::RegSet { hive, key, value, data } => live_key(*hive, key).map(|k| {
                let expect = match data {
                    RegData::Dword(d) => d.to_string(),
                    RegData::String(s) => ps_lit(s),
                };
                Check {
                    id: format!("reg:{k}\\{}", value_name(value)),
                    origin,
                    what: format!("{k}\\{} = {data}", value_name(value)),
                    script: format!(
                        "$v = Get-ItemPropertyValue -LiteralPath {path} -Name {name} -ErrorAction SilentlyContinue; $ok = ($null -ne $v) -and ($v -eq {expect}); $detail = \"value=$v\"",
                        path = ps_lit(&format!("Registry::{k}")),
                        name = ps_lit(&value_name(value)),
                    ),
                }
            }),
            Op::RegDelete { hive, key, value } => live_key(*hive, key).map(|k| Check {
                id: format!("regdel:{k}\\{value}"),
                origin,
                what: format!("{k}\\{value} is absent"),
                script: format!(
                    "$v = Get-ItemProperty -LiteralPath {path} -Name {name} -ErrorAction SilentlyContinue; $ok = ($null -eq $v); $detail = $(if ($v) {{ 'still present' }} else {{ '' }})",
                    path = ps_lit(&format!("Registry::{k}")),
                    name = ps_lit(value),
                ),
            }),
            Op::DeleteFile { path } => Some(Check {
                id: format!("file:{path}"),
                origin,
                what: format!("C:\\{path} is absent"),
                script: format!(
                    "$ok = -not (Test-Path -LiteralPath {p}); $detail = $(if ($ok) {{ '' }} else {{ 'still present' }})",
                    p = ps_lit(&format!("C:\\{path}"))
                ),
            }),
            Op::AddDriver { inf } => {
                let leaf = inf.rsplit(['\\', '/']).next().unwrap_or(inf).to_string();
                Some(Check {
                    id: format!("drv:{leaf}"),
                    origin,
                    what: format!("Driver {leaf} is in the driver store"),
                    script: format!(
                        "$d = @(Get-WindowsDriver -Online | Where-Object {{ (Split-Path $_.OriginalFileName -Leaf) -eq {n} }}); $ok = ($d.Count -gt 0); $detail = $(if ($ok) {{ $d[0].Driver }} else {{ 'not found' }})",
                        n = ps_lit(&leaf)
                    ),
                })
            }
            Op::LoadHive { .. } | Op::UnloadHive { .. } | Op::ComponentCleanup { .. } => None,
        };
        if let Some(c) = c {
            if !out.iter().any(|x: &Check| x.id == c.id) {
                out.push(c);
            }
        }
    }
    out
}

pub const RESULT_PATH: &str = r"WinISOPilot\boottest.json";

/// The in-guest script: runs every check, writes the results to
/// `C:\WinISOPilot\boottest.json`, then shuts the VM down.
pub fn verify_script(checks: &[Check]) -> String {
    let mut s = String::from(
        "$ErrorActionPreference = 'Continue'\r\n\
         $results = New-Object System.Collections.ArrayList\r\n\
         $prov = @(Get-AppxProvisionedPackage -Online | ForEach-Object { $_.DisplayName })\r\n\
         $user = @(Get-AppxPackage | ForEach-Object { $_.Name })\r\n",
    );
    for c in checks {
        s.push_str(&format!(
            "try {{ $ok = $false; $detail = ''; {script}; [void]$results.Add([pscustomobject]@{{ id = {id}; ok = [bool]$ok; detail = [string]$detail }}) }} catch {{ [void]$results.Add([pscustomobject]@{{ id = {id}; ok = $false; detail = 'check failed: ' + $_.Exception.Message }}) }}\r\n",
            script = c.script,
            id = ps_lit(&c.id)
        ));
    }
    s.push_str(&format!(
        "$os = Get-CimInstance Win32_OperatingSystem\r\n\
         $out = [pscustomobject]@{{ schema = 1; build = [string]$os.BuildNumber; caption = [string]$os.Caption; results = @($results) }}\r\n\
         New-Item -ItemType Directory -Force -Path 'C:\\WinISOPilot' | Out-Null\r\n\
         $out | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 -LiteralPath {}\r\n\
         Stop-Computer -Force\r\n",
        ps_lit(&format!("C:\\{RESULT_PATH}"))
    ));
    s
}

#[derive(Debug, Clone, Deserialize)]
struct RawResults {
    #[serde(default)]
    build: String,
    #[serde(default)]
    caption: String,
    #[serde(default)]
    results: Vec<RawResult>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawResult {
    id: String,
    ok: bool,
    #[serde(default)]
    detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    pub what: String,
    pub origin: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scored {
    pub build: String,
    pub caption: String,
    pub passed: usize,
    pub failed: Vec<CheckResult>,
    /// Checks the guest never reported.
    pub missing: Vec<String>,
}

impl Scored {
    pub fn all_passed(&self) -> bool {
        self.failed.is_empty() && self.missing.is_empty()
    }
}

/// Parses the guest's JSON (UTF-8, possibly with a BOM) and scores it.
pub fn score(json: &str, checks: &[Check]) -> Result<Scored, String> {
    let text = json.trim_start_matches('\u{feff}').trim();
    let raw: RawResults = serde_json::from_str(text).map_err(|e| format!("The test results couldn't be read: {e}"))?;
    let mut passed = 0;
    let mut failed = Vec::new();
    let mut missing = Vec::new();
    for c in checks {
        match raw.results.iter().find(|r| r.id == c.id) {
            Some(r) if r.ok => passed += 1,
            Some(r) => failed.push(CheckResult {
                what: c.what.clone(),
                origin: c.origin.clone(),
                ok: false,
                detail: r.detail.clone(),
            }),
            None => missing.push(c.what.clone()),
        }
    }
    Ok(Scored { build: raw.build, caption: raw.caption, passed, failed, missing })
}

#[cfg(test)]
mod kb_tests {
    #[test]
    fn kb_from_name() {
        assert_eq!(super::kb_number(r"D:\u\windows11.0-kb5043080-x64_ab.msu").as_deref(), Some("KB5043080"));
        assert_eq!(super::kb_number("lp.cab"), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::PlannedOp;

    fn plan() -> CompiledPlan {
        let op = |o: Op| PlannedOp { origin: "x".into(), op: o };
        CompiledPlan {
            install: vec![
                op(Op::RemoveAppx { package_name: "A_1".into(), display_name: "Clipchamp.Clipchamp".into() }),
                op(Op::RemoveCapability { name: "App.StepsRecorder~~~~0.0.1.0".into() }),
                op(Op::DisableFeature { name: "SMB1Protocol".into(), remove_payload: false }),
                op(Op::EnableFeature { name: "NetFx3".into(), source_sxs: true }),
                op(Op::LoadHive { hive: Hive::System }),
                op(Op::RegSet {
                    hive: Hive::System,
                    key: r"ControlSet001\Services\DiagTrack".into(),
                    value: "Start".into(),
                    data: RegData::Dword(4),
                }),
                op(Op::UnloadHive { hive: Hive::System }),
                op(Op::RegSet {
                    hive: Hive::DefaultUser,
                    key: r"Software\Classes\CLSID\{x}\InprocServer32".into(),
                    value: "".into(),
                    data: RegData::String("it's".into()),
                }),
                op(Op::RegDelete {
                    hive: Hive::DefaultUser,
                    key: r"Software\Run".into(),
                    value: "OneDriveSetup".into(),
                }),
                op(Op::DeleteFile { path: r"Windows\System32\Recovery\Winre.wim".into() }),
                op(Op::ComponentCleanup { reset_base: false }),
            ],
            boot: vec![],
            skipped: vec![],
        }
    }

    #[test]
    fn keys() {
        assert_eq!(setup_key("Professional"), Some("W269N-WFGWX-YVC9B-4J6C9-T83GX"));
        assert_eq!(setup_key("Core"), None);
    }

    #[test]
    fn base64_matches_reference() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        // "a" in UTF-16LE is 61 00
        assert_eq!(encode_command("a"), "YQA=");
    }

    #[test]
    fn unattend_contents() {
        let t =
            TestTarget { language: "en-US".into(), index: 1, edition_id: "Professional".into(), arch: "amd64".into() };
        let x = unattend_xml(&t);
        assert!(x.contains("<Value>1</Value>"));
        assert!(x.contains("W269N-WFGWX-YVC9B-4J6C9-T83GX"));
        assert!(x.contains("-EncodedCommand "));
        assert!(x.contains("<WillWipeDisk>true</WillWipeDisk>"));
        let home = unattend_xml(&TestTarget { edition_id: "Core".into(), ..t });
        assert!(!home.contains("<ProductKey>"));
        // Command line limit in FirstLogonCommands is 1024 characters.
        let line = x.lines().find(|l| l.contains("<CommandLine>")).unwrap();
        assert!(line.trim().len() < 1024, "{}", line.trim().len());
    }

    #[test]
    fn checks_from_plan() {
        let c = checks(&plan());
        assert_eq!(c.len(), 8);
        assert!(c.iter().any(|x| x.id == r"reg:HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Services\DiagTrack\Start"));
        assert!(c
            .iter()
            .any(|x| x.id.starts_with(r"reg:HKEY_CURRENT_USER\Software\Classes") && x.id.ends_with("(default)")));
        assert!(c.iter().any(|x| x.script.contains("'it''s'")));
        let s = verify_script(&c);
        assert!(s.contains("Stop-Computer -Force"));
        assert!(s.contains(r"C:\WinISOPilot\boottest.json"));
    }

    #[test]
    fn scoring() {
        let c = checks(&plan());
        let json = format!(
            "\u{feff}{{\"build\":\"26100\",\"caption\":\"Windows 11 Pro\",\"results\":[{}]}}",
            c.iter()
                .skip(1)
                .enumerate()
                .map(|(i, x)| format!(
                    "{{\"id\":{},\"ok\":{},\"detail\":\"d\"}}",
                    serde_json::to_string(&x.id).unwrap(),
                    i != 0
                ))
                .collect::<Vec<_>>()
                .join(",")
        );
        let s = score(&json, &c).unwrap();
        assert_eq!(s.passed, c.len() - 2);
        assert_eq!(s.failed.len(), 1);
        assert_eq!(s.missing.len(), 1);
        assert!(!s.all_passed());
        assert!(score("nope", &c).is_err());
    }
}
