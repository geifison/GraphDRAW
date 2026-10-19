; Inno Setup script for GraphDraw. Built by CI on windows-latest.
; Variables: /DVersion=x.y.z /DSrcDir=<dir with graphdraw.exe and graphdraw-cli.exe>

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef SrcDir
  #define SrcDir "..\..\target\release"
#endif

[Setup]
AppId={{97CEE5A7-A69E-4686-9E7F-BC5F50B3EF06}
AppName=GraphDraw
AppVersion={#Version}
AppPublisher=GraphDraw
AppCopyright=Copyright (c) 2026 Ighor Toth and the TraceDraw contributors and GraphDraw contributors
AppPublisherURL=https://github.com/geifison/GraphDRAW
DefaultDirName={autopf}\GraphDraw
DefaultGroupName=GraphDraw
UninstallDisplayIcon={app}\graphdraw.exe
OutputDir=..\..\dist
OutputBaseFilename=graphdraw-{#Version}-windows-x86_64-setup
SetupIconFile=..\..\assets\icon\graphdraw.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequiredOverridesAllowed=dialog
ChangesAssociations=yes
ChangesEnvironment=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "addtopath"; Description: "Add graphdraw-cli to PATH"; GroupDescription: "Command line:"

[Files]
Source: "{#SrcDir}\graphdraw.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcDir}\graphdraw-cli.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\GraphDraw"; Filename: "{app}\graphdraw.exe"
Name: "{group}\Uninstall GraphDraw"; Filename: "{uninstallexe}"
Name: "{autodesktop}\GraphDraw"; Filename: "{app}\graphdraw.exe"; Tasks: desktopicon

[Registry]
; .gdr file association
Root: HKA; Subkey: "Software\Classes\.gdr"; ValueType: string; ValueName: ""; ValueData: "GraphDraw.Document"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\GraphDraw.Document"; ValueType: string; ValueName: ""; ValueData: "GraphDraw Document"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\GraphDraw.Document\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\graphdraw.exe,0"
Root: HKA; Subkey: "Software\Classes\GraphDraw.Document\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\graphdraw.exe"" ""%1"""
; "Open with GraphDraw" for .cdr, without stealing the default
Root: HKA; Subkey: "Software\Classes\.cdr\OpenWithProgids"; ValueType: string; ValueName: "GraphDraw.Document"; ValueData: ""; Flags: uninsdeletevalue
; PATH
Root: HKA; Subkey: "{code:PathRegKey}"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Tasks: addtopath; Check: NeedsAddPath('{app}')

[Run]
Filename: "{app}\graphdraw.exe"; Description: "{cm:LaunchProgram,GraphDraw}"; Flags: nowait postinstall skipifsilent shellexec runasoriginaluser

[Code]
function PathRegKey(Param: string): string;
begin
  if IsAdminInstallMode then
    Result := 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment'
  else
    Result := 'Environment';
end;

function NeedsAddPath(Param: string): boolean;
var
  OrigPath: string;
  Root: Integer;
begin
  if IsAdminInstallMode then Root := HKLM else Root := HKCU;
  if not RegQueryStringValue(Root, PathRegKey(''), 'Path', OrigPath) then
  begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Uppercase(ExpandConstant(Param)) + ';', ';' + Uppercase(OrigPath) + ';') = 0;
end;
