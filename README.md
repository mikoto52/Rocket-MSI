# Express MSI Builder

개발 코드명은 **Express MSI Builder**로 유지한다. 정식 이름 후보는 **InstallEmbly**, **InstallPackora**이며 아직 확정하지 않았다. 후보의 기존 사용 및 상표 등록·출원 상태는 확인 전이다. 현재 프로그램 표시명, 기본 BrandingText, 프로젝트 확장자 `.embproj`는 유지한다.

폴더와 제품 정보를 입력하고 레지스트리 및 설치 옵션을 설정하여 Windows Installer 패키지(`.msi`)를 만드는 Windows 데스크톱 프로그램.

프로그램 이름은 **Express MSI Builder**이며 프로젝트 파일은 XML 기반 **`.embproj`** 형식이다. [프로젝트 예제](examples/MyApp.embproj)는 현재 엔진으로 빌드할 수 있다. [형식 및 시험 안내](docs/embproj-v1.md)를 참고한다.

현재 단계는 **MSI 엔진 및 Tauri 빌더 구현·검증**이다. Rust CLI와 Tauri 편집기가 `.embproj`에서 한글 설치 마법사를 포함하는 MSI를 생성한다. [빌더 사용 안내](docs/builder-gui.md)와 [업그레이드 시험](docs/upgrade-test.md)을 참고한다.

## 첫 구현 실행

Windows x64, Rust MSVC 도구 체인 및 x86/x64 Visual C++ 빌드 도구가 필요하다. 실행 도우미 DLL을 위해 `rustup target add i686-pc-windows-msvc x86_64-pc-windows-msvc`로 두 타깃을 준비한다. CAB 생성에는 해당 Windows의 `makecab.exe`를 사용한다. XML 파서로 xml-rs 0.8.29를 사용하며 WiX 의존성은 없다. 처음에는 `cargo fetch --locked`로 의존성을 준비해야 한다.

프로젝트 기반 빌드:

```powershell
cargo run --offline -- init MyProject.embproj
cargo run --offline -- validate examples/MyApp.embproj
cargo run --offline -- build examples/MyApp.embproj
```

프로젝트에서 제품 정보·GUID·아키텍처·설치 경로·약관·브랜딩·CAB·완료 후 실행·레지스트리·바로가기·설치/제거 실행 목록을 설정한다. 소스·약관·출력 상대 경로는 프로젝트 폴더 기준이다. `<Upgrade enabled="true" />`로 동일 계열의 버전 업그레이드와 버전 충돌 차단을 활성화한다. 실제 업그레이드 시험은 별도 절차를 따른다.

설치/제거 실행 목록은 순서·인수·창 숨김·작업 폴더·시간 제한과 오류 시 무시/알림 후 계속/중단을 지원한다. [설정과 시험 키트](docs/actions-test.md)를 참고한다.

시작 메뉴·바탕화면 바로가기는 설치된 파일을 대상으로 인수·작업 폴더를 지정할 수 있다. [프로젝트 예제](examples/ShortcutDemo.embproj)와 [시험 안내](docs/shortcut-test.md)를 참고한다.

레지스트리는 `REG_SZ`, `REG_DWORD`, `REG_BINARY`, `REG_MULTI_SZ`와 기본 경로 변수 치환을 지원한다. [프로젝트 예제](examples/RegistryDemo.embproj) 및 [설치·복구·제거 시험](docs/registry-test.md)을 참고한다.

`Installation existingFiles="default|skip|overwrite"`로 기존 파일의 기본 규칙·건너뛰기·덮어쓰기를 선택한다. 설치와 마법사 복구에 적용하며 파일별 예외 및 GUI는 후속 구현이다. [정책 및 시험 키트](docs/file-policy-test.md)를 참고한다.

```powershell
cargo test --offline
cargo run --offline -- build examples/payload/hello.txt dist/emb-demo.msi
```

약관과 하단 브랜딩을 포함하려면:

```powershell
cargo run --offline -- build examples/payload/hello.txt dist/my-terms.msi --license examples/demo-terms.txt --branding-text "Express MSI Builder"
```

TXT/RTF 입력과 동의 정책은 [약관 및 브랜딩 명세](docs/license-and-branding.md)를 참고한다.

설치 완료 후 실행은 `--launch-file bin/MyApp.exe --launch-arg "인수 하나"`로 설정한다. 실행 파일은 입력 폴더에 포함되어야 한다. 완료 화면의 체크박스는 기본 해제이며, 무인 설치나 유지보수 중에는 실행하지 않는다. [완료 후 실행 시험](docs/finish-launch-test.md)을 참고한다.

CAB를 MSI 밖에 저장하려면 `--cab-storage external`을 지정한다. 기본값은 `embedded`다. 외부 모드에서는 출력 MSI와 같은 폴더의 `Data1.cab`을 함께 배포해야 한다. 각 패키지는 별도 출력 폴더를 사용한다. [출력 방식 및 시험](docs/cab-packaging.md)을 참고한다.

`--cab-size-mib N`을 추가하면 크기별로 Data1.cab, Data2.cab 등으로 나눈다. 외부 모드에서는 생성된 CAB를 모두 함께 배포한다. 단일 파일의 CAB 경계 걸침은 아직 지원하지 않는다. [분할 CAB 시험](docs/split-cab-test.md)을 참고한다.

폴더 전체를 포함하려면 소스 인수에 폴더를 지정한다.

```powershell
cargo run --offline -- build examples/multi-payload dist/my-folder/Setup.msi --cab-storage external --license examples/demo-terms.txt
```

명령은 MSI를 생성할 뿐 설치하지 않는다. 같은 출력 파일이 이미 있으면 덮어쓰지 않는다. 기존 소스·출력 직접 지정 명령은 고정 시험용 제품 정보를 유지한다. 제품 정보 편집은 `.embproj`를 사용한다. 단일 파일 입력은 기존 호환을 위해 설치 이름 hello.txt를 유지하며, 폴더 입력은 원래 파일 이름과 하위 경로를 유지한다. 현재는 개발 검증 단계다. [다중 파일 시험 안내](docs/multi-file-test.md)를 참고한다.

구현 언어는 **Rust**, 데스크톱 UI는 **Tauri**로 정한다. MSI 생성기는 Windows Installer API를 이용해 자체 구현하며, CAB 생성은 Windows 기능을 활용한다. 프런트엔드 프레임워크와 정확한 의존성 버전은 아직 미정이다.

## 배포 방침

Express MSI Builder는 무료로 배포하고 소스코드도 공개한다. 구체적인 소스코드 라이선스는 아직 선정하지 않았으며, 공개 저장소에 올리는 것과 수정·재배포 권한을 부여하는 라이선스 선정은 구분한다.

외부 의존성의 비용·사용·재배포 조건은 별도로 검토한다. 빌더가 무료라는 사실이 의존 도구의 비용 조건을 면제하는 것은 아니다. MSI 생성 백엔드에 WiX를 사용하지 않는다. 빌더 자체의 배포 과정에도 WiX가 자동으로 포함되지 않도록 별도로 검증한다.

## 문서

- [제품 요구사항](docs/requirements.md): 범위, 화면 흐름, 완료 조건
- [변수 및 레지스트리 명세](docs/variables-and-registry.md): 변수 문법, 평가 시점, 데이터 형식, 수명 주기
- [설치·제거 프로그램 실행 명세](docs/program-actions.md): 여러 실행 항목, 인수, 창 숨김, 실행 순서와 실패 처리
- [구현 설계](docs/architecture.md): 기술 후보, 프로젝트 저장 형식, MSI 생성 구조
- [개발 및 검증 계획](docs/development-plan.md): 단계별 산출물, 기술 검증, 설치 시험
- [복구·제거 시험](docs/maintenance-test.md): 유지보수 동작과 시험 절차
- [CAB 출력·분할 설계](docs/cab-packaging.md): MSI 내장/외부 CAB 선택과 후속 분할 계획

## 문서 상태

작성일: 2026-10-17. 버전: 0.1.

사용자가 요청한 핵심은 간편한 MSI 생성, 레지스트리 설정, 설치 대상 PC의 값을 사용하는 변수 기능이다. 세부 기본값과 기술 선택은 문서에 제안으로 명시했으며, 구현 전 기술 검증 결과에 따라 갱신한다.

문서만으로 개발을 준비하며 외부 배포, 도구 구매, 시스템 레지스트리 변경은 수행하지 않았다.
