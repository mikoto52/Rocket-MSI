# .embproj v1 — 현재 구현

XML 프로젝트 생성·검증·저장·MSI 빌드를 지원한다. UTF-8(선택적 BOM)을 사용하며 루트는 ExpressMsiBuilderProject, schemaVersion="1"이다. 실행 가능한 예제는 `examples/MyApp.embproj`다. 이전 전체 설계는 `docs/full-project-design.xml`로 보존했으며 현재 엔진에서는 빌드할 수 없다.

```powershell
cargo run --offline -- init MyProject.embproj
cargo run --offline -- validate examples/MyApp.embproj
cargo run --offline -- build examples/MyApp.embproj
cargo run --offline -- normalize MyProject.embproj MyProject.normalized.embproj
```

init은 새 ProductCode·UpgradeCode를 파일에 저장한다. 기본 소스는 프로젝트 옆 payload 폴더다. validate는 구조와 설정만 검사하며 파일 존재 여부와 CAB 생성 등은 빌드 때 검사한다. normalize는 현재 지원 설정을 새 파일로 저장한다. 주석·원래 서식은 보존하지 않으며 상대 경로 문자열은 유지한다. 따라서 다른 폴더에 저장할 때는 경로를 수정해야 한다. 프로젝트와 빌드 출력은 기존 파일을 덮어쓰지 않는다.

| 요소 | 속성 및 내용 |
|---|---|
| Product (필수) | name, manufacturer, version, productCode, upgradeCode, architecture=x86/x64, scope=perMachine |
| Source (필수) | directory: 재귀 포함할 폴더 |
| Installation (필수) | directory, allowDirectoryChange=true/false, existingFiles=default/skip/overwrite (선택) |
| Registry (선택) | Entry 반복: REG_SZ, REG_DWORD, REG_BINARY, REG_MULTI_SZ. [명세 및 시험](registry-test.md) |
| Shortcuts (선택) | Shortcut 반복: 시작 메뉴/바탕화면, 대상 파일·인수·작업 폴더. [명세 및 시험](shortcut-test.md) |
| Actions (선택) | Install/Uninstall의 Action 목록: 인수·창 숨김·오류 정책. [현재 명세 및 시험](actions-test.md) |
| Upgrade (선택) | enabled=true/false. [현재 제한과 시험](upgrade-test.md). 새 프로젝트는 true, 요소 생략 시 false |
| UI (선택) | License, BrandingText, FinishLaunch |
| UI/License | file: UTF-8 TXT 또는 RTF |
| UI/BrandingText | 한 줄 텍스트. 기본 Express MSI Builder |
| UI/FinishLaunch | file: 소스 내부 상대 EXE 경로 |
| FinishLaunch/Arguments | Argument 요소를 순서대로 반복. 요소 하나가 인수 하나 |
| Build (필수) | outputDirectory, outputFile: 경로 없는 .msi 파일명 |
| Build/Cabinet (선택) | storage=embedded/external, split=none/size, size일 때 targetSizeMiB=1..2047 |

Cabinet 생략 시 내장·분할 없음이다. split=none에는 targetSizeMiB를 쓰지 않는다. 외부 CAB는 MSI 옆 Data1.cab, Data2.cab 등을 모두 배포한다. 단일 파일을 여러 CAB에 걸쳐 나누지는 않는다.

소스·약관·출력 상대 경로는 **프로젝트 파일 폴더 기준**이다. FinishLaunch.file은 소스 기준이며 설치된 파일을 실행한다. 인수는 빈 문자열·양끝 공백·XML 이스케이프를 왕복 보존한다.

```xml
<FinishLaunch file="bin/MyApp.exe">
  <Arguments>
    <Argument>first argument</Argument>
    <Argument>[literal]</Argument>
    <Argument></Argument>
  </Arguments>
</FinishLaunch>
```

완료 화면의 실행 체크박스는 기본 해제이며 /passive, /qn, 복구·제거에서는 실행하지 않는다.

Installation.directory는 `{ProgramFiles}\`로 시작하며 하위 폴더에서 `{Manufacturer}`, `{ProductName}`, `{ProductVersion}`을 치환한다. x64는 ProgramFiles64Folder, x86은 ProgramFilesFolder를 사용한다. allowDirectoryChange=false는 경로 입력을 비활성화한다. 관리자 명령줄의 INSTALLDIR 재정의를 금지하는 보안 기능은 아니다. ProgramData 등 다른 변수는 후속 단위다.

GUID는 대문자·중괄호로 정규화한다. 잘못된 GUID·0 GUID·동일한 ProductCode와 UpgradeCode는 거부한다. 버전 상한은 255.255.65535이며 숫자 세 부분이 필요하다. PackageCode는 빌드마다 생성한다. 컴포넌트 GUID는 UpgradeCode·아키텍처·설치 위치 설정·파일 상대 경로를 기반으로 결정하며 동일 설정의 재빌드에서 유지된다.

**자동 업그레이드는 Upgrade enabled=true로 활성화한다.** UpgradeCode 유지·버전 증가·새 ProductCode가 필요하며 동일 아키텍처·설치 위치 구성으로 제한한다. 예전 패키지의 내부 업그레이드 정보가 없는 경우와 위치 변경은 차단한다. [업그레이드 명세](upgrade-test.md)를 따른다.

알 수 없는 요소·속성, 중복 단일 요소, 미지원 스키마, DTD·엔티티 선언·네임스페이스·처리 명령은 거부한다. XML 크기 1 MiB, 요소 10,000개, 깊이 64로 제한한다. [Tauri 편집기](builder-gui.md)에서도 동일한 검증을 사용한다.

## 수동 시험

1. 예제를 빌드한다. dist/project-demo 출력이 이미 있다면 outputDirectory를 다른 폴더로 바꾼다.
2. dist/project-demo/MyApp.msi를 실행해 이름 MyApp, 약관·브랜딩을 확인한다.
3. 기본 경로 `C:\Program Files\MyCompany\MyApp`에 파일 네 개가 하위 경로대로 설치되는지 확인한다.
4. 같은 MSI를 다시 열어 복구·제거를 시험한다. 이 예제는 이전 고정 시험용 제품과 다른 GUID다.
5. 배포 시 MyApp.msi와 Data1.cab을 함께 둔다.

자동 테스트는 XML 왕복·잘못된 입력·상대 경로·x86/x64 실제 MSI 테이블과 설치 세션 열기를 검사한다. 실제 설치·제거는 자동 실행하지 않는다.

기존 파일 처리 정책은 [명세 및 시험 키트](file-policy-test.md)를 참고한다. 생략 시 MSI 기본 규칙을 적용하며, 프로젝트 기반 마법사 복구에도 정책이 적용된다.


## 명시적 파일 목록과 실행 시점 확장 (2026-10-19)

Source와 Files 중 정확히 하나를 지정한다. 기존 Source 전체 폴더 방식은 호환된다. Files는 서로 다른 원본에서 고른 파일을 대상 경로로 배치한다. source는 프로젝트 기준 상대 경로 또는 절대 경로다.

```xml
<Files>
  <File source="payload/app.exe" target="{InstallDestination}\bin\app.exe" />
  <File source="other/helper.dll" target="{SystemDir}\helper.dll" />
  <File source="data/config.txt" target="{ProgramFiles}\Vendor\config.txt" />
  <File source="data/probe.txt" target="{SystemDrive}\Vendor\probe.txt" />
  <File source="data/temp.txt" target="{TempDir}\Vendor\temp.txt" />
</Files>
```

대상 루트 변수는 InstallDestination, ProgramFiles, SystemDir, SystemDrive, TempDir이다. 변수가 없는 target은 InstallDestination 기준이다. target은 역슬래시로 구분하며 상위 경로, 중복, 파일/디렉터리 충돌은 거부한다. 파일이 없는 목록은 저장할 수 있지만 MSI 빌드에는 파일이 하나 이상 필요하다. 빈 폴더 자체를 설치하는 기능은 포함하지 않는다.

Action의 선택 속성 timing은 Install에서 beforeInstall / beforeFiles / afterFiles / beforeFinish, Uninstall에서 beforeRemove다. 생략하면 Install=afterFiles, Uninstall=beforeRemove다. 같은 시점은 XML 순서를 유지한다. 기존 설치 항목은 이제 afterFiles(InstallFiles 바로 뒤)로 실행되어 레지스트리 기록보다 먼저 실행될 수 있다. 레지스트리 등 나머지 설치 리소스가 필요한 작업은 beforeFinish를 지정한다.

GUI는 Executable.kind=targetPath를 사용하며 변수/절대 경로를 저장한다. installedFile은 XML 호환용으로 유지하되 복사 전 시점에서는 사용할 수 없다. targetPath의 대상 누락은 런타임 실패 정책을 따른다. InstallDestination, SystemDir, SystemDrive, TempDir 변수는 실행 경로·인수·작업 폴더와 레지스트리 문자열 데이터에도 지원한다. Registry/Entry.root에 HKCR을 추가했다.
