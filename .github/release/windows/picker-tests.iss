{ The Release workflow appends these tests to the production code and wraps its
  InitializeWizard. The installer logic and game-data readers stay in their
  production owners. Abort after writing the result prevents installation. }

procedure AssertPicker(Value: Boolean; Description: String);
begin
  if not Value then
    RaiseException(Description);
end;

procedure RunPickerTests();
var
  Candidates: TArrayOfString;
  First, Second, Manual, ReportText, TestKey: String;
begin
  First := NormalizeGameDir(ExpandConstant('{src}\picker fixtures\尤里 one'));
  Second := NormalizeGameDir(ExpandConstant('{src}\picker fixtures\game two'));
  Manual := NormalizeGameDir(ExpandConstant('{src}\picker fixtures\manual folder'));
  AssertPicker(ForceDirectories(First) and ForceDirectories(Second), 'create fixtures');
  AddCandidate(Candidates, First);
  AddCandidate(Candidates, Uppercase(First) + '\.\');
  AddCandidate(Candidates, Second);
  AddCandidate(Candidates, First + '\');
  AddCandidate(Candidates, First + '\not present');
  AssertPicker(GetArrayLength(Candidates) = 2, 'dedup, existing-only, damaged-copy discovery');
  TestKey := 'Software\VERA20K-Picker-Tests';
  try
    AssertPicker(RegWriteStringValue(HKCU, TestKey, 'InstallPath', First), 'write test folder registration');
    AssertPicker(RegistryDir(HKCU, TestKey, 'InstallPath', True) = First, 'registry folder preserved');
    AssertPicker(RegWriteStringValue(HKCU, TestKey, 'InstallPath', First + '\gamemd.exe'),
      'write test executable registration');
    AssertPicker(RegistryDir(HKCU, TestKey, 'InstallPath', True) = First, 'registry executable parent');
  finally
    RegDeleteKeyIncludingSubkeys(HKCU, TestKey);
  end;

  SetArrayLength(Installations, 2);
  Installations[0].Folder := First;
  Installations[0].Language := '0';
  Installations[0].RequiredIssues := 'cameomd.mix: unavailable';
  Installations[0].OptionalIssues := '';
  Installations[1].Folder := Second;
  Installations[1].Language := '2';
  Installations[1].RequiredIssues := '';
  Installations[1].OptionalIssues := 'test-song.wav: unavailable';
  RecommendInstallation();
  AssertPicker(RecommendedInstall = 1, 'valid later copy recommended');
  AssertPicker(PreferredGameDir('') = Second, 'recommended default');
  AssertPicker(PreferredGameDir(First) = First, 'explicit invalid choice preserved');
  AssertPicker(PreferredGameDir(Manual) = Manual, 'explicit undetected choice preserved');
  ReportText := InstallationReport(Installations[1]);
  AssertPicker((Pos(Second, ReportText) > 0) and
    (Pos(CustomMessage('GameLanguage2'), ReportText) > 0) and
    (Pos('test-song.wav', ReportText) > 0), 'path, CSF language, optional missing content');
  AssertPicker(Pos('cameomd.mix', InstallationReport(Installations[0])) > 0, 'required missing content');
  AssertPicker(GameLanguageName('') = CustomMessage('GameLanguageUnknown'), 'unknown language');
  AssertPicker(GameLanguageName('10') = CustomMessage('GameLanguageUnknown'), 'explicit unknown language');
  AssertPicker(Pos('1049', GameLanguageName('1049')) > 0, 'unrecognized language remains unknown');
  Installations[0].RequiredIssues := '';
  Installations[0].OptionalIssues := 'optional.wav: unavailable';
  Installations[1].OptionalIssues := '';
  RecommendInstallation();
  AssertPicker(RecommendedInstall = 1, 'prefer complete optional content');
  Installations[1].OptionalIssues := 'optional.wav: unavailable';
  RecommendInstallation();
  AssertPicker(RecommendedInstall = 0, 'stable tie order');
  Installations[0].RequiredIssues := 'missing required data';
  Installations[1].RequiredIssues := 'missing required data';
  RecommendInstallation();
  AssertPicker(RecommendedInstall = -1, 'never recommend a failing installation');
  Installations[1].RequiredIssues := '';
  RecommendInstallation();

  InstallationsScanned := True;
  GameDirPage.Values[0] := Second;
  RefreshInstallationList();
  AssertPicker(GameInstallList.Items.Count = 2, 'all installation rows');
  AssertPicker(GameInstallList.Checked[1], 'recommended radio selection');
  AssertPicker(Pos(CustomMessage('GameInstallRecommended'), GameInstallList.ItemSubItem[1]) > 0,
    'recommended row annotation');
  AssertPicker(GameInstallList.Height >= ScaleY(48), 'list has usable height');
  AssertPicker(GameInstallDetails.Height >= ScaleY(48), 'details have usable height');
  AssertPicker(GameInstallDetails.Top + GameInstallDetails.Height <= GameDirPage.SurfaceHeight,
    'details fit standard wizard');
  GameDirPage.Values[0] := Manual;
  AssertPicker(not GameInstallList.Checked[0] and not GameInstallList.Checked[1], 'manual edit clears radio');
  AssertPicker(GameInstallDetails.Text = CustomMessage('GameInstallManual'), 'manual edit clears stale content');
  UpdatingSelection := True;
  GameInstallList.Checked[0] := True;
  UpdatingSelection := False;
  GameInstallationSelected(nil);
  AssertPicker(GameDir() = First, 'radio selection updates actual folder');
  AssertPicker(Pos('missing required data', GameInstallDetails.Text) > 0, 'radio updates details');
  CurPageChanged(GameDirPage.ID);
  AssertPicker(NextButtonClick(GameCheckPage.ID), 'optional report permits Next');
  RequiredIssues := 'required missing';
  CurPageChanged(GameCheckPage.ID);
  AssertPicker(not WizardForm.NextButton.Enabled, 'required report disables Next');
  AssertPicker(not NextButtonClick(GameCheckPage.ID), 'required report blocks programmatic Next');
end;

procedure InitializeWizard();
begin
  try
    ProductionInitializeWizard();
    RunPickerTests();
    SaveStringToFile(ExpandConstant('{src}\picker-test-result.txt'), 'passed', False);
  except
    SaveStringToFile(ExpandConstant('{src}\picker-test-result.txt'), 'failed: ' + GetExceptionMessage, False);
  end;
  Abort;
end;
