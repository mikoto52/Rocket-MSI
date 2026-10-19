# Rocket MSI

Rocket MSI는 Windows Installer 패키지(`.msi`)를 만드는 **Windows 전용 설치 빌더**입니다. Rust로 구현한 자체 MSI 엔진과 Tauri 2 기반 GUI를 사용하며, WiX에 의존하지 않습니다. 프로젝트는 XML 기반 **`.embproj`** 파일로 저장합니다.

현재는 기능 구현과 검증을 진행하는 개발 버전입니다.

## 주요 기능

| 기능 | 현재 지원 범위 |
|---|---|
| 제품 설정 | 제품명·제작사·버전, x86/x64, ProductCode·UpgradeCode 관리 |
| 설치 파일 | 펼치기/접기 트리, 파일 다중 선택, 디렉터리 전체 가져오기, 대상 폴더별 배치·이동·삭제 |
| 설치 화면 | 한글 마법사, 환영·완료·중단의 좌측 이미지, 배너 화면, 전체 문구 BrandingText, TXT/RTF 사용권 계약 동의 |
| Quick MSI 첫 단위 | Embedded UI DLL 포함, 간편 설치·진행·완료·유지보수 화면, 설치 후 실행 선택. 실제 설치 검증 진행 중 |
| 유지보수 | 아이콘이 있는 복구·제거 선택과 작업별 확인 페이지, 기존 파일의 MSI 기본 규칙·건너뛰기·덮어쓰기 정책 |
| 레지스트리 | HKLM·HKCU·HKCR, 32/64비트 영역, REG_SZ·REG_DWORD·REG_BINARY·REG_MULTI_SZ, 키 트리/값 목록 |
| 바로가기 | 시작 메뉴·바탕화면, 설치된 대상 파일, 인수·작업 폴더·경로 변수 |
| 프로그램 실행 | 설치·제거 중 여러 명령, 실행 시점·인수·작업 폴더·창 숨김·시간 제한, 실패 시 무시/알림/중단 |
| 설치 완료 후 실행 | 완료 화면에서 선택한 설치 파일 실행; 무인 설치·복구·제거 중에는 실행하지 않음 |
| 업그레이드 | 이전 버전 교체, 다운그레이드·동일 버전 중복 설치 차단, 실패 시 기존 버전 롤백 |
| CAB | MSI 내장 또는 외부 저장, 크기별 Data1.cab·Data2.cab 분할, makecab 창 숨김 실행 |
| 빌드 도구 | GUI 및 독립 콘솔 CLI, 프로젝트 검사·빌드·MSI 검증 |

설치 파일의 대상 루트는 **InstallDestination, ProgramFiles, ProgramData, WindowsDir, SystemDir, SystemDrive, TempDir**입니다. ProgramFiles·SystemDir은 패키지의 x86/x64에 맞게 해석합니다.

프로그램 경로·작업 폴더·레지스트리 문자열 등에 경로 및 제품 정보 변수를 사용할 수 있습니다. 지원하는 입력란 옆 **`<` 버튼**을 누르면 변수 메뉴가 열립니다. 바로가기의 기본 대상은 `{InstallDestination}\file.exe`입니다. 기존 프로젝트의 `{InstallDir}` 변수는 열 때 `{InstallDestination}`으로 변환합니다.

설치 명령의 실행 시점은 **설치 시작 전 / 파일 복사 전 / 파일 복사 후 / 설치 완료 전**이며, 같은 시점에서는 목록 순서대로 실행합니다. 제거 명령은 파일 삭제 전에 실행합니다. 복구·업그레이드 중에는 사용자 명령을 실행하지 않습니다.

## GUI 사용

배포용 실행 파일은 빌드 후 `dist/builder`에 생성됩니다.

- **RocketMsiBuilder.exe**: 프로젝트를 편집하고 MSI를 만드는 GUI
- **RocketMsiBuilderCli.exe**: `.embproj`를 빌드하는 콘솔 도구

GUI에는 **Microsoft Edge WebView2 Runtime**이 필요합니다. CLI에는 WebView2가 필요하지 않습니다. 두 도구 모두 Windows의 `makecab.exe`를 사용합니다.

1. GUI에서 새 프로젝트를 만들고 제품 정보를 설정합니다.
2. 설치 파일 트리에서 대상 폴더를 선택한 뒤 파일 또는 디렉터리를 추가합니다.
3. 약관·레지스트리·바로가기·실행 목록을 설정합니다.
4. 빌드 화면에서 출력 위치와 CAB 저장/분할 방식을 지정합니다.
5. `.embproj`를 저장하고 프로젝트 검사 및 MSI 빌드를 실행합니다.

디렉터리 가져오기는 현재 하위 파일 목록을 고정하므로 이후 원본에 추가한 파일은 다시 넣어야 합니다. 목록에서 삭제해도 원본 파일은 삭제하지 않습니다. 기존 `<Source directory="..."/>` 프로젝트도 읽고 빌드할 수 있습니다.

상세 사용법은 [빌더 안내](docs/builder-gui.md)를 참고하세요.

브랜딩 기본값은 **Rocket MSI**입니다. 설정한 문구는 하단뿐 아니라 환영·진행·완료·중단 화면의 마법사 이름에도 적용됩니다. 하단 텍스트박스는 빌드 시 실제 글자 폭과 여유 폭을 반영해 자동 조정합니다. 화면 흐름과 버튼 표기는 [설치 UI 안내](docs/installer-ui.md)를 참고하세요.

## CLI 사용

`dist/builder`에서 실행하는 예입니다.

```powershell
.\RocketMsiBuilderCli.exe --help
.\RocketMsiBuilderCli.exe init "D:\projects\MyProject.embproj"
.\RocketMsiBuilderCli.exe validate "D:\projects\MyProject.embproj"
.\RocketMsiBuilderCli.exe build "D:\projects\MyProject.embproj"

# build 명령을 생략해도 됩니다.
.\RocketMsiBuilderCli.exe "D:\projects\MyProject.embproj"

# 생성한 MSI의 패키지 구조를 확인합니다. 설치하지 않습니다.
.\RocketMsiBuilderCli.exe verify "D:\projects\dist\Setup.msi"
```

`init`으로 생성한 프로젝트는 원본 폴더 등 설정을 수정한 뒤 빌드합니다. 소스·약관·출력의 상대 경로는 **실행 위치가 아닌 프로젝트 파일의 폴더**를 기준으로 해석합니다. 출력 위치와 파일명은 프로젝트의 `Build` 설정을 따릅니다.

종료 코드는 성공 **0**, 오류 **1**입니다. 기존 출력 MSI/CAB는 덮어쓰지 않으므로 출력 폴더를 구분하세요. 외부 CAB 방식은 MSI와 CAB 파일 전체를 함께 배포해야 합니다. 빌드는 MSI나 사용자 명령을 실행하지 않습니다.

[프로젝트 예제](examples/MyApp.embproj)와 [XML 형식](docs/embproj-v1.md)을 참고하세요.

## 소스에서 빌드

처음 환경을 준비하는 방법부터 배포용 GUI·CLI 생성, MSI 빌드와 오류 해결까지는 **[빌드 안내](docs/build.md)**에 정리되어 있습니다.

개발 환경은 Windows x64와 Rust MSVC 도구 체인입니다. Visual C++ x86/x64 빌드 도구 및 Windows SDK가 필요합니다. 프런트엔드는 로컬 HTML/CSS/JavaScript이며 Node.js/npm 서버 없이 배포합니다.

```powershell
rustup target add i686-pc-windows-msvc x86_64-pc-windows-msvc
cargo fetch --locked
cargo fetch --manifest-path builder/Cargo.toml --locked

# GUI·CLI 및 의존성 라이선스 안내를 dist/builder에 생성합니다.
powershell -NoProfile -File scripts/build-builder.ps1
```

MSI 엔진은 Windows Installer API를 직접 사용하고 CAB는 Windows makecab으로 생성합니다. Tauri 번들러는 비활성화되어 있으며 이 빌드 과정에서도 WiX를 사용하지 않습니다. GUI 자체를 설치하는 패키지는 아직 제공하지 않습니다.

개발 중에는 Cargo로도 프로젝트를 검사하고 빌드할 수 있습니다.

```powershell
cargo run --offline -- validate examples/MyApp.embproj
cargo run --offline -- build examples/MyApp.embproj
```

## 검증

```powershell
cargo test --offline
cargo test --offline --manifest-path builder/Cargo.toml
cargo clippy --offline --all-targets -- -D warnings
cargo clippy --offline --manifest-path builder/Cargo.toml --all-targets -- -D warnings
powershell -NoProfile -File scripts/test-builder-cli.ps1
```

자동 검증은 실제 MSI 생성·재열기, 디렉터리 경로 계산, CAB·레지스트리·바로가기·실행 시퀀스, 프로젝트 저장과 CLI 빌드를 포함하며 설치는 실행하지 않습니다.

수동 시험에서 설치·복구·제거, 레지스트리·바로가기, 실행 실패 정책, passive/quiet 실행, 버전 업그레이드·충돌 차단·실패 롤백을 확인했습니다. 최근 추가한 GUI 편집 기능과 실행 시점·특수 대상 폴더 조합은 별도의 실제 설치 검증이 필요합니다.

## 현재 제한과 후속 작업

- **Quick MSI 첫 단위**를 빌더와 `<UI mode="quick">`에서 선택할 수 있습니다. 약관 표시·경로 편집·사용자 제품 아이콘은 아직 제공하지 않으며 실제 설치·실행 분기의 수동 검증이 필요합니다. [설정과 시험 안내](docs/quick-msi.md)
- 현재 설치 범위는 모든 사용자(`perMachine`)입니다. 빌더는 MSI를 생성하며 설치 권한 상승을 처리하는 Setup.exe 부트스트래퍼는 후속 과제입니다.
- 일부 덮어쓰기 복구 시험에서 일반 권한 실행의 롤백 정리 오류가 남아 있습니다. 관리자 실행은 통과했으며 [기록](docs/development-plan.md)을 참고하세요.
- 업그레이드는 같은 UpgradeCode, 아키텍처와 설치 경로 구성의 제품을 대상으로 합니다. 새 버전에는 버전 증가와 새 ProductCode가 필요합니다. 설치 위치를 옮기는 업그레이드는 지원하지 않습니다.
- 개별 파일 2 GiB 이상과 파일 하나를 여러 CAB에 걸쳐 분할하는 방식은 지원하지 않습니다.
- 빈 폴더만 설치하기, 파일별 덮어쓰기 예외, 빌드 취소, 사용자 이미지 편집, 코드 서명, 드래그앤드롭은 아직 제공하지 않습니다.
- 사용자 명령이 외부에 만든 변경은 MSI가 자동으로 되돌리지 않습니다. 숨김 옵션도 실행 프로그램이 자체 생성한 모든 창을 숨긴다는 보장은 하지 않습니다.

## 배포 및 라이선스

Rocket MSI는 무료 배포와 소스 공개를 목표로 개발합니다. 프로젝트 자체의 소스코드 라이선스는 아직 선정하지 않았습니다. 외부 의존성의 라이선스 안내는 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)와 [빌더 의존성 안내](builder/THIRD-PARTY-NOTICES.md)를 참고하세요.

## 관련 문서

- [빌드 안내](docs/build.md)
- [빌더 사용 안내](docs/builder-gui.md)
- [프로젝트 XML 형식](docs/embproj-v1.md)
- [약관·브랜딩](docs/license-and-branding.md)
- [설치 UI와 유지보수 흐름](docs/installer-ui.md)
- [Quick MSI 첫 단위와 시험](docs/quick-msi.md)
- [변수·레지스트리](docs/variables-and-registry.md) / [레지스트리 시험](docs/registry-test.md)
- [바로가기 시험](docs/shortcut-test.md)
- [설치·제거 실행](docs/program-actions.md) / [실행 시험](docs/actions-test.md)
- [완료 후 실행 시험](docs/finish-launch-test.md)
- [CAB 출력 방식](docs/cab-packaging.md) / [분할 CAB 시험](docs/split-cab-test.md)
- [기존 파일 정책](docs/file-policy-test.md)
- [업그레이드 시험](docs/upgrade-test.md)
- [요구사항](docs/requirements.md) / [구현 설계](docs/architecture.md) / [개발 계획](docs/development-plan.md)
