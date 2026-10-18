# Tauri 빌더 첫 구현

Windows 전용 Tauri 2 데스크톱 앱이다. UI는 로컬 HTML/CSS/JavaScript로 내장하며 Node/npm 서버가 실행 시 필요하지 않다. 새 프로젝트·열기·저장·다른 이름 저장·검사·MSI 빌드를 제공한다. 엔진을 Rust 라이브러리로 공유하므로 CLI와 동일한 XML 검증 및 MSI 생성 코드를 사용한다.

화면은 제품 정보, 설치 파일, 설치 화면, 레지스트리, 바로가기, 실행 목록, 빌드로 나뉜다. 레지스트리 네 형식, 시작 메뉴·바탕화면, 설치/제거 실행 항목의 인수 목록·작업 폴더·창 숨김·무시/알림/중단·실행 순서도 편집한다. 업그레이드 활성화와 GUID 관리, 다음 버전 준비(마지막 버전 숫자 증가 및 새 ProductCode 발급)를 제공한다.

## 실행

완성된 실행 파일은 `dist/builder/ExpressMsiBuilder.exe`에 둔다. Windows WebView2 Runtime이 필요하다. 앱은 관리자 권한으로 실행할 필요가 없으며 프로그램을 빌드만 한다. MSI 실제 설치는 사용자가 별도로 진행한다.

소스에서 실행/검사:

```powershell
cargo run --manifest-path builder/Cargo.toml --offline
cargo test --manifest-path builder/Cargo.toml --offline
cargo clippy --manifest-path builder/Cargo.toml --offline -- -D warnings
```

최초에는 의존성 다운로드가 필요하다. Cargo.lock에 해석된 버전을 고정한다. Rust x86/x64 MSVC 타깃과 Visual C++ 빌드 도구도 기존 엔진과 동일하게 필요하다. 배포용 EXE는 `cargo build --manifest-path builder/Cargo.toml --release --offline`로 생성한다. Tauri 번들러는 비활성화하여 WiX를 사용하지 않는다. GUI 자체의 MSI 패키징은 후속 작업이다.

## 사용 순서

1. 새 프로젝트를 만들거나 기존 .embproj를 연다.
2. 제품 이름·제작사·버전·원본 폴더를 설정한다. 원본은 폴더 전체를 포함하며 파일 제외 편집은 아직 제공하지 않는다.
3. 약관·레지스트리·바로가기·실행 목록을 필요에 따라 추가한다.
4. 출력 폴더와 MSI 파일명을 지정하고 저장한다.
5. 프로젝트 검사 후 MSI 빌드를 누른다. 기존 출력 MSI/CAB가 있으면 덮어쓰지 않고 오류를 표시한다.

상대 경로는 프로젝트 파일 기준이다. 다른 폴더에 다른 이름으로 저장하면 기존 소스·약관·출력 참조를 절대 경로로 변환해 의미를 유지한다. 새 프로젝트는 처음 저장하는 위치가 상대 경로 기준이다. 외부에서 파일이 바뀌면 기존 문서 저장/빌드를 중단하며, 새로 열거나 다른 이름으로 저장해야 한다. 저장은 임시 파일을 완성한 뒤 교체하며 저장 중 실패 시 기존 문서를 보존한다.

빌드 중에는 입력을 잠그고 상태 메시지를 표시한다. 빌드 취소·실시간 상세 makecab 출력·파일 트리·사용자 이미지 편집·서명·드래그앤드롭은 후속 기능이다. 빌드 기록은 현재 세션의 상태/오류 메시지다. 기존 XML 주석·서식은 정규화 저장 시 유지하지 않는다.

## 검증 상태

Rust 저장 테스트와 컴파일·Clippy, JavaScript 구문 검사 및 브라우저 미리보기의 제품 정보 반영·실행 항목 편집·다중 문자열 전환을 확인했다. 실제 Windows 앱 실행도 확인했으나 앱 UI 자동 조작은 접근 승인 시간 초과로 완료하지 못했다. 따라서 네이티브 파일 대화상자→저장→빌드의 전체 GUI 수동 시험은 필요하다.

GUI 예제 시험은 `examples/MyApp.embproj`를 열고 새 outputDirectory를 지정한 후 다른 이름으로 저장·검사·빌드하는 순서로 진행한다. 설치·제거 실행의 시험 프로그램은 앱 편집 중이나 빌드 중 실행하지 않는다.

참고: [Tauri 전제 조건](https://v2.tauri.app/start/prerequisites/), [Rust 명령 호출](https://v2.tauri.app/develop/calling-rust/).
