# Rocket MSI 빌드 안내

정식 표시 이름과 기본 브랜딩은 Rocket MSI다. 저장소 주소 `mikoto52/InstallEmbly`, 내부 Cargo 패키지명 `install-embly-builder`, `.embproj` 형식은 현재 유지한다. 아래 복제 경로와 내부 실행 파일명을 정식 이름으로 임의 변경하지 않는다. 배포 파일명은 `RocketMsiBuilder.exe`와 `RocketMsiBuilderCli.exe`다.

이 문서는 Windows에서 소스를 준비하고 GUI·CLI 실행 파일을 만드는 방법을 설명합니다. 빌드 스크립트는 실행 파일을 생성하며 프로그램 설치나 MSI 설치는 실행하지 않습니다.

## 1. 개발 환경

현재 개발/빌드 대상은 **Windows x64 + Rust MSVC**입니다.

- Git
- Rust 및 Cargo: 기본 호스트 `x86_64-pc-windows-msvc`
- Visual Studio 또는 Visual Studio Build Tools의 **Desktop development with C++** 구성 요소
  - MSVC x86/x64 컴파일러와 링커
  - Windows SDK: 실행 파일 아이콘을 컴파일하는 `rc.exe`도 필요합니다.
- Windows PowerShell 5.1 이상
- GUI 실행용 Microsoft Edge WebView2 Runtime

CLI 실행에는 WebView2가 필요하지 않습니다. 생성된 MSI는 프로젝트 설정에 따라 x86 또는 x64를 대상으로 합니다. 빌더 실행 파일 자체의 아키텍처와 MSI 대상 아키텍처는 구분합니다.

CAB는 Windows의 `makecab.exe`를 사용합니다. WiX, Node.js, npm은 이 빌드에 필요하지 않습니다. 프런트엔드는 로컬 HTML/CSS/JavaScript이며 Tauri 번들러는 비활성화되어 있습니다.

## 2. 소스 및 의존성 준비

새로 내려받는 경우:

```powershell
git clone https://github.com/mikoto52/InstallEmbly.git
cd InstallEmbly
```

기존 작업 폴더가 있다면 해당 저장소 루트에서 아래 명령을 실행합니다.

```powershell
rustc --version
cargo --version
rustup target add i686-pc-windows-msvc x86_64-pc-windows-msvc

cargo fetch --locked
cargo fetch --manifest-path builder/Cargo.toml --locked
```

첫 의존성 다운로드에는 네트워크가 필요합니다. 이후 아래 빌드 스크립트는 캐시와 Cargo.lock을 사용해 `--offline`으로 빌드합니다. 두 Rust 타깃은 x86/x64 MSI 실행 도우미 DLL을 함께 만들기 위해 필요합니다.

## 3. GUI·CLI 배포용 빌드

저장소 루트에서 실행합니다. 관리자 권한은 필요하지 않습니다.

```powershell
powershell -NoProfile -File scripts/build-builder.ps1
```

| 출력 | 용도 |
|---|---|
| `dist/builder/RocketMsiBuilder.exe` | GUI 프로젝트 편집기 및 MSI 빌더 |
| `dist/builder/RocketMsiBuilderCli.exe` | `.embproj` 빌드용 콘솔 도구 |
| `dist/builder/dependencies.json` | 의존성 버전·라이선스 목록 |
| `dist/builder/THIRD-PARTY-NOTICES.md` | 외부 의존성 안내 |
| `dist/builder/LICENSE` | Rocket MSI의 MIT 라이선스 |
| `dist/builder/licenses/` | 의존성 라이선스·고지 파일 |

배포할 때는 라이선스·고지 파일도 함께 포함합니다. Windows의 makecab이나 WebView2를 이 스크립트에서 복사·설치하지는 않습니다. GUI 자체의 설치용 MSI/Setup.exe는 아직 만들지 않습니다.

개별 컴파일 명령은 아래와 같습니다. 배포 파일명 정리와 라이선스 수집까지 하려면 위 스크립트를 사용하세요.

```powershell
cargo build --release --offline
# target/release/emb-msi.exe → 배포명 RocketMsiBuilderCli.exe

cargo build --manifest-path builder/Cargo.toml --release --offline
# builder/target/release/install-embly-builder.exe → 배포명 RocketMsiBuilder.exe
```

## 4. 개발 실행

GUI 디버그 빌드와 실행:

```powershell
cargo run --manifest-path builder/Cargo.toml --offline
```

CLI 디버그 실행:

```powershell
cargo run --offline -- --help
cargo run --offline -- validate examples/MyApp.embproj
cargo run --offline -- build examples/MyApp.embproj
```

GUI는 프로젝트를 저장한 뒤 MSI를 빌드합니다. 경로와 예제 사용법은 [빌더 사용 안내](builder-gui.md)를 참고하세요.

## 5. embproj에서 MSI 생성

아래는 **Rocket MSI 자체를 컴파일하는 명령이 아니라**, 완성된 CLI로 프로젝트의 설치 패키지를 만드는 명령입니다.

```powershell
.\dist\builder\RocketMsiBuilderCli.exe --help
.\dist\builder\RocketMsiBuilderCli.exe init "D:\projects\MyProject.embproj"

# 생성한 프로젝트의 제품 정보·Source/Files·출력 위치를 먼저 설정합니다.
.\dist\builder\RocketMsiBuilderCli.exe validate "D:\projects\MyProject.embproj"
.\dist\builder\RocketMsiBuilderCli.exe build "D:\projects\MyProject.embproj"

# 프로젝트 경로만 넘겨도 빌드합니다.
.\dist\builder\RocketMsiBuilderCli.exe "D:\projects\MyProject.embproj"

# 설치 없이 패키지 구조를 확인합니다.
.\dist\builder\RocketMsiBuilderCli.exe verify "D:\projects\dist\Setup.msi"
```

- 소스·약관·출력 상대 경로의 기준은 **프로젝트 파일이 있는 폴더**입니다.
- 출력 위치/파일명은 `.embproj`의 `Build` 설정을 따릅니다.
- 기존 출력 MSI/CAB를 덮어쓰지 않으므로 새 출력 폴더나 파일명을 사용합니다.
- 외부 CAB 방식에서는 MSI와 모든 DataN.cab를 함께 배포합니다.
- CLI 종료 코드는 성공 0, 오류 1입니다.
- GUI·CLI 모두 makecab을 창 없이 실행하고 출력을 캡처합니다. CLI 자체의 콘솔 출력은 유지합니다.

## 6. 검증

```powershell
cargo test --offline
cargo test --offline --manifest-path builder/Cargo.toml
cargo clippy --offline --all-targets -- -D warnings
cargo clippy --offline --manifest-path builder/Cargo.toml --all-targets -- -D warnings

# 배포용 CLI를 먼저 빌드한 뒤 실행합니다.
powershell -NoProfile -File scripts/test-builder-cli.ps1
```

CLI 시험은 도움말·초기화·설정 검사·MSI 생성·프로젝트 경로 직접 실행·MSI 검증과 오류 종료 코드를 확인합니다. 기존 출력/잘못된 XML 오류는 의도적인 시험이며 마지막 PASS를 확인합니다. 시험 파일은 `target/cli-smoke-*`에 남깁니다.

위 자동 검증은 실제 프로그램 설치·복구·제거를 실행하지 않습니다. 설치 수명 주기는 [업그레이드 시험](upgrade-test.md), [실행 시험](actions-test.md), [레지스트리 시험](registry-test.md), [바로가기 시험](shortcut-test.md) 절차로 별도 확인합니다.

## 7. 큐브 아이콘 및 설치 이미지 재생성

이미지와 아이콘은 저장소에 포함되어 있어 일반 빌드 전에 재생성할 필요가 없습니다. 도형을 변경할 때 아래 명령을 사용합니다.

```powershell
powershell -NoProfile -File scripts/generate-installer-art.ps1
powershell -NoProfile -File scripts/generate-app-icon.ps1
powershell -NoProfile -File scripts/build-builder.ps1
```

두 생성기는 `scripts/draw-package.ps1`의 동일한 큐브 도형을 공유합니다. `assets/installer/app-icon.png`는 투명 PNG이며 `app-icon.ico`는 16/24/32/48/64/128/256픽셀 프레임을 포함합니다. 앱 아이콘 생성기는 GUI 리소스와 화면의 아이콘 파일도 함께 갱신합니다. CLI는 tauri-winres로 Windows 아이콘 리소스를 컴파일합니다.

## 8. 자주 만나는 빌드 문제

| 증상 | 확인할 내용 |
|---|---|
| offline에서 패키지를 찾지 못함 | 두 Cargo.toml에 대해 `cargo fetch --locked`를 먼저 실행했는지 확인 |
| i686 타깃/실행 도우미 DLL 빌드 실패 | Rust 두 타깃과 MSVC x86/x64 빌드 도구 설치 확인 |
| link.exe / rc.exe를 찾지 못함 | Visual C++ 빌드 도구 및 Windows SDK 설치 확인; 필요하면 VS의 Developer PowerShell에서 실행 |
| EXE 복사 시 접근 거부 | 실행 중인 빌더를 닫고 다시 빌드 |
| GUI 실행 시 WebView2 오류 | WebView2 Runtime 설치 상태 확인 |
| MSI/CAB 출력이 이미 존재함 | embproj의 출력 폴더/파일명을 새 위치로 변경 |
| 탐색기에 이전 아이콘이 보임 | 실행 파일을 다시 열고 탐색기 아이콘 캐시 표시와 실제 리소스를 구분 |

프로젝트 자체는 [MIT 라이선스](../LICENSE)로 배포하며, 의존성 안내는 [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md)를 참고하세요.
