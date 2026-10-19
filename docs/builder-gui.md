# Rocket MSI 빌더 사용 안내

기본 브랜딩은 Rocket MSI이며 설치 화면 설정에서 변경한다. 같은 문구가 하단과 마법사 이름에 반영된다. 설치 UI의 최신 흐름과 유지보수 확인 단계는 [설치 UI 안내](installer-ui.md)를 따른다. Quick MSI 첫 단위는 화면 방식 선택에서 제공하며 제한과 검증 절차는 [Quick MSI 안내](quick-msi.md)를 따른다.

앱 아이콘은 `assets/installer`의 기존 큐브 도형을 사용한다. 같은 생성 함수를 공유해 GUI/CLI EXE 아이콘, 창/작업 표시줄 아이콘, 화면의 로고와 패키지 아이콘을 통일했다. PNG는 투명 배경이고 ICO는 16~256픽셀 여러 크기를 포함한다. `scripts/generate-app-icon.ps1`로 다시 생성한다.

Windows 전용 Tauri 2 데스크톱 앱이다. UI는 로컬 HTML/CSS/JavaScript로 내장하며 Node/npm 서버가 실행 시 필요하지 않다. 새 프로젝트·열기·저장·다른 이름 저장·검사·MSI 빌드를 제공한다. 엔진을 Rust 라이브러리로 공유하므로 CLI와 동일한 XML 검증 및 MSI 생성 코드를 사용한다.

화면은 제품 정보, 설치 파일, 설치 화면, 레지스트리, 바로가기, 실행 목록, 빌드로 나뉜다. 레지스트리 네 형식, 시작 메뉴·바탕화면, 설치/제거 실행 항목의 인수 목록·작업 폴더·창 숨김·무시/알림/중단·실행 순서도 편집한다. 업그레이드 활성화와 GUID 관리, 다음 버전 준비(마지막 버전 숫자 증가 및 새 ProductCode 발급)를 제공한다.

Quick MSI를 만들려면 왼쪽 **설치 화면**에서 **설치 화면 방식 → 설치 화면 → Quick MSI**를 선택한 뒤 저장하고 빌드한다. 일반 마법사는 **마법사**를 선택한다. 선택값은 `.embproj`의 `<UI mode="quick">` 또는 `<UI mode="wizard">`에 저장되며 프로젝트를 다시 열어도 유지된다. 모드가 없는 기존 프로젝트는 마법사를 사용한다. Quick MSI에서는 현재 사용권 계약 표시를 지원하지 않으므로 약관 파일을 지정하면 빌드를 차단한다.

## 실행

프로젝트 파일을 실행 인수로 전달하면 파일 선택 창 없이 해당 프로젝트를 연다. 인수가 없으면 새 프로젝트로 시작한다. 공백·한글 경로는 따옴표로 감싼다. 여러 파일이나 잘못된 파일을 지정하면 오류를 표시하고 새 프로젝트 화면을 제공한다.

```powershell
.\RocketMsiBuilder.exe "C:\Users\User\Documents\내 프로젝트.embproj"
```

설치 프로그램에서 파일 연결을 작성할 때 `RocketMSI.Project\shell\open\command`의 기본값은 `"{InstallDestination}\RocketMsiBuilder.exe" "%1"`로 지정한다. `.embproj`의 기본값은 `RocketMSI.Project`로 연결한다. 빌더 자체 실행은 시스템 파일 연결을 변경하지 않는다. 프로젝트의 Registry에 추가할 예시는 다음과 같다. Windows의 사용자 기본 앱 선택 정책에 따라 연결 적용 여부는 별도로 확인한다.

```xml
<Registry>
  <Entry id="project_extension" root="HKCR" key=".embproj" name="" type="REG_SZ"><Data>RocketMSI.Project</Data></Entry>
  <Entry id="project_description" root="HKCR" key="RocketMSI.Project" name="" type="REG_SZ"><Data>Rocket MSI 프로젝트</Data></Entry>
  <Entry id="project_open" root="HKCR" key="RocketMSI.Project\shell\open\command" name="" type="REG_SZ"><Data>"{InstallDestination}\RocketMsiBuilder.exe" "%1"</Data></Entry>
</Registry>
```

완성된 실행 파일은 `dist/builder/RocketMsiBuilder.exe`에 둔다. Windows WebView2 Runtime이 필요하다. 앱은 관리자 권한으로 실행할 필요가 없으며 프로그램을 빌드만 한다. MSI 실제 설치는 사용자가 별도로 진행한다.

창 너비 1200px 이하에서는 오른쪽 요약 패널을 본문 상단의 간단한 작업 패널로 전환한다. MSI 빌드·프로젝트 검사·출력 폴더 열기 버튼은 유지한다. 넓은 창에서는 기존 오른쪽 요약 패널을 사용한다.

소스에서 실행/검사:

```powershell
cargo run --manifest-path builder/Cargo.toml --offline
cargo test --manifest-path builder/Cargo.toml --offline
cargo clippy --manifest-path builder/Cargo.toml --offline -- -D warnings
```

최초에는 의존성 다운로드가 필요하다. Cargo.lock에 해석된 버전을 고정한다. Rust x86/x64 MSVC 타깃과 Visual C++ 빌드 도구도 기존 엔진과 동일하게 필요하다. 배포용 EXE는 `cargo build --manifest-path builder/Cargo.toml --release --offline`로 생성한다. Tauri 번들러는 비활성화하여 WiX를 사용하지 않는다. GUI 자체의 MSI 패키징은 후속 작업이다.

## 사용 순서

1. 새 프로젝트를 만들거나 기존 .embproj를 연다.
2. 제품 이름·제작사·버전을 설정한다. 설치 파일 화면 왼쪽에서 대상 폴더를 선택한 뒤 파일·디렉터리를 추가하고, 오른쪽 목록에서 이동·삭제한다.
3. 약관·레지스트리·바로가기·실행 목록을 필요에 따라 추가한다.
4. 출력 폴더와 MSI 파일명을 지정하고 저장한다.
5. 프로젝트 검사 후 MSI 빌드를 누른다. 기존 출력 MSI/CAB가 있으면 덮어쓰지 않고 오류를 표시한다.

상대 경로는 프로젝트 파일 기준이다. 다른 폴더에 다른 이름으로 저장하면 기존 소스·약관·출력 참조를 절대 경로로 변환해 의미를 유지한다. 새 프로젝트는 처음 저장하는 위치가 상대 경로 기준이다. 외부에서 파일이 바뀌면 기존 문서 저장/빌드를 중단하며, 새로 열거나 다른 이름으로 저장해야 한다. 저장은 임시 파일을 완성한 뒤 교체하며 저장 중 실패 시 기존 문서를 보존한다.

빌드 중에는 입력을 잠그고 상태 메시지를 표시한다. 빌드 취소·실시간 상세 makecab 출력·사용자 이미지 편집·서명·드래그앤드롭은 후속 기능이다. 빌드 기록은 현재 세션의 상태/오류 메시지다. 기존 XML 주석·서식은 정규화 저장 시 유지하지 않는다.

## 검증 상태

Rust 저장 테스트와 컴파일·Clippy, JavaScript 구문 검사 및 브라우저 미리보기의 제품 정보 반영·실행 항목 편집·다중 문자열 전환을 확인했다. 실제 Windows 앱 실행도 확인했으나 앱 UI 자동 조작은 접근 승인 시간 초과로 완료하지 못했다. 따라서 네이티브 파일 대화상자→저장→빌드의 전체 GUI 수동 시험은 필요하다.

GUI 예제 시험은 `examples/MyApp.embproj`를 열고 새 outputDirectory를 지정한 후 다른 이름으로 저장·검사·빌드하는 순서로 진행한다. 설치·제거 실행의 시험 프로그램은 앱 편집 중이나 빌드 중 실행하지 않는다.

참고: [Tauri 전제 조건](https://v2.tauri.app/start/prerequisites/), [Rust 명령 호출](https://v2.tauri.app/develop/calling-rust/).


## 2026-10-19 편집 화면 개선

- 설치 파일: 왼쪽 대상 폴더 트리, 오른쪽 선택한 폴더의 파일 목록. 여러 파일 추가, 디렉터리 전체 가져오기, 대상 폴더 생성, 선택 파일 이동·삭제를 제공한다.
- 대상 루트: InstallDestination, ProgramFiles, SystemDir, SystemDrive, TempDir, ProgramData, WindowsDir. ProgramFiles/SystemDir은 x86/x64에 맞춰 해석한다. SystemDrive는 Windows가 설치된 볼륨이다. TempDir은 MSI TempFolder이며 고정된 Windows 하위 Temp를 뜻하지 않는다.
- 디렉터리 가져오기는 폴더 이름과 하위 파일 구조를 유지한다. 파일 경로를 참조하는 명시적 목록이므로 이후 원본에 생긴 파일은 다시 추가해야 한다. 목록 삭제는 원본을 지우지 않는다. 빈 대상 폴더는 파일을 넣은 뒤 저장되며, 빈 원본 폴더만 가져오기는 지원하지 않는다.
- 새 GUI 프로젝트는 Files를 사용한다. 기존 Source 프로젝트도 읽고 빌드할 수 있으며 구성 불러오기로 현재 파일 목록을 고정할 수 있다. 두 방식을 동시에 저장하지 않는다.
- 레지스트리: 왼쪽 루트/키 트리, 오른쪽 값 이름·형식·영역·데이터 목록. HKCR을 지원하며 현재 perMachine 설치에서는 HKLM\Software\Classes에 기록한다.
- 실행 항목의 대상 종류 선택을 제거했다. 프로그램 경로와 실행 시점을 설정하며 내부 XML은 targetPath를 저장한다. 상대 경로는 InstallDestination 아래로 해석한다.
- 실행 시점: 설치 시작 전, 파일 복사 전, 파일 복사 후, 설치 완료 전. 실제 실행은 시점 우선, 같은 시점에서는 목록 순서다. 제거는 파일 삭제 전이다. 복구·업그레이드 중에는 실행하지 않는다.
- 실행 프로그램·인수 및 레지스트리 문자열/다중 문자열에 변수 삽입 버튼을 제공한다. 커서/선택 영역에 삽입한다. DWORD/바이너리는 숫자/HEX 입력이다.
- 제공 변수: InstallDestination, ProgramFiles, SystemDir, SystemDrive, TempDir, ProgramData, WindowsDir, ProductName, Manufacturer, ProductVersion. 작업 폴더에도 변수를 직접 입력할 수 있다.

자동 검증은 x86/x64 실제 MSI 생성, 특수 폴더 경로 계산(설치 없이), HKCR Root=0, 실행 시점, 잘못된 대상 경로 거부를 포함한다. 브라우저 미리보기에서 특수 루트 트리, 대상 종류 제거, 시점 선택, 프로그램/레지스트리 변수 삽입과 키별 값 표시를 확인했다. 네이티브 파일 선택과 실제 설치·복구·제거는 추가 수동 검증 대상이다.


### 계층형 트리 및 변수 통일

파일/레지스트리는 중첩된 부모·자식 트리로 표시한다. 자식이 있는 항목은 화살표로 접고 펼칠 수 있고 폴더/키 선택과 펼침 버튼이 분리되어 있다. 펼침 상태는 섹션 이동 중 유지한다. 새 하위 폴더/키로 이동할 때는 조상 노드를 자동으로 펼친다.

파일 대상에 ProgramData(CommonAppDataFolder), WindowsDir(WindowsFolder)을 추가했다. 설치 경로 변수의 공식 이름은 InstallDestination이며 InstallDir은 변수 선택 목록과 엔진 변수에서 제거했다. 기존 프로젝트의 유효한 InstallDir 템플릿은 열 때 InstallDestination으로 변환하여 저장한다. 이스케이프한 리터럴 중괄호와 원본 파일 경로는 바꾸지 않는다. MSI 내부 INSTALLDIR 및 업그레이드 메타데이터 식별자는 호환성을 위해 유지한다.


### 바로가기 대상 및 명령 작업 폴더 변수

바로가기 추가의 설치할 대상 파일 기본값은 `{InstallDestination}\file.exe`다. 입력란 옆 변수 삽입 버튼으로 대상 루트 및 제품 정보 변수를 넣을 수 있다. 예를 들어 `{InstallDestination}\{ProductName}.exe`처럼 사용한다. 대상 파일은 설치 파일 목록에 포함되어 있어야 하며, MSI는 해당 파일 ID를 참조해 실제 설치 위치를 따른다. ProgramData/WindowsDir 등 다른 루트에 배치한 파일도 같은 루트 변수 경로로 지정한다. 기존 상대 대상 경로도 호환한다.

실행 명령어 편집 창의 작업 폴더에도 변수 삽입 버튼을 제공한다. 설치·제거 항목 모두 같은 동작이며 현재 커서/선택 영역에 삽입한다. 비워 두면 실행 파일의 폴더를 사용한다.


변수 버튼은 `<`로 표시하며 툴팁/접근성 이름은 변수 삽입이다. 클릭하면 버튼 근처에 컨텍스트 메뉴 형태의 변수 목록이 나타난다. 선택 시 커서/선택 영역에 삽입하며 바깥 클릭과 Esc로 닫힌다. 위/아래 화살표, Home/End, Enter로 선택할 수 있다. 별도 모달 창은 사용하지 않는다.


## CLI 배포 및 makecab 창 숨김

`scripts/build-builder.ps1`은 `dist/builder/RocketMsiBuilder.exe`(GUI), `dist/builder/RocketMsiBuilderCli.exe`(콘솔 CLI)를 함께 생성한다. CLI는 GUI/WebView2 없이 동일한 Rust 엔진으로 embproj를 빌드한다. makecab은 두 빌더 모두 CREATE_NO_WINDOW로 실행하고 출력/오류를 캡처한다.

```powershell
.\RocketMsiBuilderCli.exe --help
.\RocketMsiBuilderCli.exe validate "D:\path\MyProject.embproj"
.\RocketMsiBuilderCli.exe build "D:\path\MyProject.embproj"
# build 생략도 지원
.\RocketMsiBuilderCli.exe "D:\path\MyProject.embproj"
```

CLI 종료 코드는 성공 0, 오류 1이다. 상대 경로는 실행 위치가 아닌 프로젝트 폴더를 기준으로 해석한다. 출력 위치/파일명은 embproj의 Build 설정을 따르고 기존 출력은 덮어쓰지 않는다. CLI 자체는 콘솔 출력을 유지하며 MSI를 설치하거나 사용자 실행 항목을 실행하지 않는다. init/normalize/verify 및 기존 원본 경로 직접 빌드 명령도 유지한다.
