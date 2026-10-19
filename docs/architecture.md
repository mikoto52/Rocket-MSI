# 구현 설계안

## 1. 기술 선택

배포 방침은 **무료 배포 및 소스코드 공개**로 확정되었다. 프로젝트 자체의 라이선스는 미정이다. 기술 후보는 이 방침에 맞는 재배포 및 사용자 측 사용 조건까지 확인한 뒤 확정한다.

구현 언어는 **Rust**, 데스크톱 UI는 **Tauri**로 정한다. 이전 C#/WinForms 방향을 대체한다. 화면은 HTML/CSS 및 JavaScript 또는 TypeScript로 작성하고 Rust 명령을 호출한다. 프런트엔드 프레임워크와 정확한 Tauri/Rust 의존성 버전은 별도 확정한다.

MSI 생성기는 Windows Installer API를 호출하는 자체 백엔드로 구현한다. Windows Installer 자체를 재구현하지 않는다. CAB는 초기 검증에 OS의 makecab.exe를 활용할 수 있으며, 최종 FCI API 직접 호출을 우선 검토한다. Windows 시스템 바이너리를 복사해 동봉하는 방식은 전제하지 않는다.

Tauri의 Windows 화면은 WebView2를 사용한다. 이 의존성은 빌더 실행에 해당하며 생성한 MSI의 설치 UI나 실행 도우미에 Tauri/WebView2를 요구하지 않도록 분리한다. 빌더는 일반 사용자 권한으로 실행하고 생성 MSI의 설치 권한 상승과 구분한다.

WiX는 생성 백엔드에서 사용하지 않는다. Tauri의 빌더 자체 패키징 경로도 별도로 확인하여 WiX 기반 MSI 번들을 자동 사용하지 않는다. 초기 개발은 설치 번들을 생성하지 않는 실행 파일 빌드를 기준으로 하고, 자체 MSI 생성기가 준비된 뒤 빌더 배포에 활용할 수 있다.

근거: [Tauri 소개](https://v2.tauri.app/start/), [Rust 명령 호출](https://v2.tauri.app/develop/calling-rust/), [Windows 사전 요구사항](https://v2.tauri.app/start/prerequisites/).

## 2. 모듈 경계

```text
Desktop UI (Tauri / Web frontend)
    → Tauri Commands: 입력 검증 및 Rust 코어 호출
        → Application: 프로젝트 열기/저장, 검증, 빌드 조정
        → Domain: 제품, 파일, 레지스트리, 변수, 프로그램 실행 항목, 진단
        → Persistence: .embproj XML 저장, 스키마 이전
        → MSI Backend: 중간 모델 → Windows Installer API + CAB → MSI
        → Build Infrastructure: 작업 폴더, 프로세스, 취소, 로그
```

UI는 MSI XML을 직접 생성하지 않는다. 도메인 검증과 변수 파서는 UI 및 빌드 백엔드와 독립적으로 시험 가능하게 한다. 백엔드는 검증된 중간 모델만 받는다.

## 3. 빌드 흐름

1. 프로젝트를 스냅샷으로 고정하고 필수 입력·파일·변수·레지스트리를 검증한다.
2. 프로젝트 외부 참조 및 제외 규칙을 적용해 대상 파일 목록을 고정한다. 출력/작업 폴더를 입력 폴더에 재포함하지 않는다.
3. 파일 및 레지스트리 컴포넌트의 안정적인 ID와 GUID를 결정한다.
4. 문자열 리터럴 이스케이프와 변수 변환을 거쳐 MSI 테이블 및 실행 시퀀스를 구성한다.
5. CAB를 생성·포함하고 MSI 데이터베이스를 커밋한 뒤 구조 검사 및 사용 가능한 ICE 검증을 실행한다.
6. 성공한 결과만 최종 출력 위치로 옮기고 소스, 도구 버전 및 로그를 연결한다.

작업마다 별도 임시 폴더를 사용한다. 취소 시 해당 작업의 자식 프로세스를 종료하고 정리한다. 기존 성공 MSI를 실패한 빌드로 덮어쓰지 않는다. 명령행 인자는 문자열 셸 연결 대신 프로세스 인자 API로 전달한다.

## 4. MSI 설계 원칙

- 동일 제품 계열의 UpgradeCode는 유지한다. 메이저 업그레이드용 ProductCode와 빌드 결과 PackageCode의 변경 규칙을 구분한다.
- 버전 증가를 확인하고 하위 버전 설치를 차단한다. 같은 버전 재빌드는 자동 업그레이드로 간주하지 않는다.
- 설치 파일과 레지스트리는 MSI 표준 테이블 및 컴포넌트로 관리한다. 무조건적인 직접 쓰기 스크립트를 생성하지 않는다.
- 각 컴포넌트의 리소스 소유권과 KeyPath를 명시한다. 유지 레지스트리를 삭제 컴포넌트와 혼합하지 않는다.
- 동적 날짜·드라이브 계산은 필요한 경우에만 보조 동작을 사용한다. 계산과 실제 레지스트리 쓰기를 분리해 표준 MSI 롤백을 활용한다.
- 기본 설치 UI와 무인 설치에서 실행 시퀀스가 동일한 결과를 내야 한다.
- 레지스트리 키 경로 변수도 실제 MSI 필드에서 지원되는지 별도 검증한다. 네이티브 치환이 안 되는 위치는 직접 지원된다고 가정하지 않는다.
- 설치·제거 프로그램 목록을 실행 시퀀스에 통합한다. 창 숨김 및 인수 전달은 [프로그램 실행 명세](program-actions.md)를 따르며 별도 실행 도우미의 필요성을 검증한다.

## 5. 프로젝트 저장 형식 초안

프로그램의 공식 이름은 **Rocket MSI**이며 프로젝트 파일은 XML 기반 **`.embproj`** 형식이다. 현재 구현된 구조는 [v1 명세](embproj-v1.md)를 기준으로 한다. 아래 확장 설계에는 미구현 항목이 포함된다.

실제 XML 예제는 [MyApp.embproj](../examples/MyApp.embproj)에 둔다. 최상위 요소는 `ExpressMsiBuilderProject`이며 `schemaVersion="1"`로 형식 버전을 구분한다.

| 요소 | 내용 |
|---|---|
| Product | 패키징할 제품 정보; 빌더 이름과 구분 |
| Source | 소스 폴더, 제외 항목, 주 실행 파일 |
| Installation / Shortcuts | 설치 위치와 바로가기 |
| Variables / Variable | 사용자 변수 이름과 템플릿 |
| Registry / Entry | 레지스트리 항목; Data 또는 Items/Item으로 데이터 표현 |
| Actions / Install / Action | 설치 실행 항목, 문서 순서대로 실행 |
| Actions / Uninstall / Action | 제거 실행 항목, 문서 순서대로 실행 |
| Build | MSI 출력 경로 |

Build 설정은 [CAB 출력 및 분할 설계](cab-packaging.md)를 따른다. 현재 CLI와 XML 로더는 CAB 내장/외부 저장 및 크기별 분할을 지원한다.

UTF-8 XML 선언, 들여쓰기, 일관된 요소 순서를 사용한다. 요소·속성 이름은 대소문자를 구분한다. 문자열은 XML 라이브러리로 이스케이프하며 인수 및 데이터의 공백은 보존한다. 빈 인수는 `<Argument />`로 표현한다. `timeoutSeconds` 생략은 제한 없음을 뜻한다. DWORD는 Data의 정수 텍스트, 다중 문자열은 Items/Item 목록으로 표현한다.

로더는 DTD와 외부 엔터티 처리를 금지하며 네트워크에서 스키마를 읽지 않는다. XML 구문 오류는 줄·열로, 의미 오류는 항목 ID와 필드로 보고한다. 미지원 요소·속성은 조용히 유실시키지 않고 오류로 처리한다. 정식 XSD와 이전 버전 마이그레이션 규칙은 코어 모델 단계에서 작성한다.

상대 경로는 프로젝트 파일이 있는 디렉터리를 기준으로 해석한다.

사용자 프로젝트에는 도구 머신의 임시 경로나 설치 시점 값을 저장하지 않는다. GUID 예시는 복사해서 모든 제품에 재사용하지 않으며 프로젝트 생성 시 실제 값을 만든다. 알 수 없는 미래 스키마는 덮어쓰지 않고 명시적으로 열기 오류를 표시한다. 저장은 임시 파일 작성 후 교체하여 중단 시 원본 손상을 줄인다.

## 6. 구현 전 결정 기록

| 항목 | 초기 방향 | 확정 기준 |
|---|---|---|
| UI | Tauri | Rust 전환과 함께 기존 WinForms 대체 |
| 언어 | Rust | 컴파일러 및 의존성 버전 고정 |
| MSI 백엔드 | 자체 구현 | 최소 MSI 및 수명 주기 검증 |
| 빌더 지원 OS | Windows 11 x64 우선 제안 | 개발/시험 환경 및 대상 사용자 |
| 설치일자 상태 | 내부 제품 상태에 저장 | 복구·업그레이드·롤백 시험 |
| 동적 키 경로 | MSI 지원 확인 후 변환 | 특수 문자 및 무인 설치 시험 |
| 유지 레지스트리 | 독립 컴포넌트 | 제거 및 업그레이드 잔존 시험 |


## 7. Rust 개발 구조안

- `apps/desktop`: Tauri 호스트와 웹 프런트엔드. 화면 편집과 진행 상태 표시 담당.
- `crates/emb-core`: 프로젝트 모델, 변수, 의미 검증. Tauri에 의존하지 않음.
- `crates/emb-project`: .embproj XML 읽기·쓰기와 버전 처리.
- `crates/emb-msi`: MSI 테이블 및 시퀀스 생성, CAB 통합.
- `crates/emb-win32`: Windows Installer/FCI API 래퍼. unsafe와 핸들 관리를 이 경계에 한정.
- `crates/emb-action-runner`: 설치·제거 프로그램 실행 도우미. UI 및 WebView2와 독립.

위 디렉터리는 개발 구조안이며 아직 구현되지 않았다. 먼저 코어와 최소 MSI 생성기를 검증한 뒤 편집 UI를 연결한다. 긴 빌드는 작업 스레드에서 수행하고 진행·취소를 Tauri 경계에서 전달한다. 웹 화면에는 임의 셸 실행을 노출하지 않으며 프로젝트 입력은 Rust에서 다시 검증한다. 빌드 및 파일 접근 명령은 해당 로컬 창에 필요한 범위로 제한한다.

## 8. GUID 편집과 업그레이드 정책

사용자 요구에 따라 제품 정보의 고급 설정에서 GUID를 조회·복사·수정하고 새 GUID를 생성할 수 있게 한다. 일반 사용자는 자동 관리를 사용하도록 한다. 기본 업그레이드 방식은 MSI major upgrade이며 제품 버전의 첫 숫자가 그대로인 1.0.0 → 1.1.0에도 적용할 수 있다.

| 식별자 | 기본 관리 규칙 |
|---|---|
| UpgradeCode | 프로젝트 생성 시 발급하고 동일 제품 계열에서 유지; 직접 수정 시 기존 제품 자동 연결이 끊길 수 있음을 표시 |
| ProductCode | 프로젝트에 저장하고 major upgrade용 새 버전 준비 시 새 GUID 발급; 같은 릴리스 빌드마다 임의 변경하지 않음; 수동 지정 가능 |
| PackageCode | 빌드 결과 식별자; 매 빌드 새 GUID 생성 및 결과 로그 표시; 초기 UI에서 수동 편집하지 않음 |
| Component GUID | 엔진이 MSI 컴포넌트 규칙에 맞게 안정적으로 관리; 초기 UI에서 수동 편집하지 않음 |

XML Product에 `productCode` 속성을 추가한다. ProductVersion 변경 시 ProductCode 갱신과 해당 버전의 연결을 프로젝트에 저장한다. 이전에 배포한 GUID를 다른 릴리스에 재사용하지 않도록 마지막 릴리스 정보 관리 방식을 코어 구현에서 확정한다. 입력 GUID는 파싱 후 MSI 형식인 대문자 중괄호 표기로 정규화한다.

UpgradeCode가 같다는 사실만으로 기존 버전이 제거되지는 않는다. Upgrade 테이블, 관련 제품 검색, 하위 버전 차단 및 RemoveExistingProducts의 실행 순서를 함께 작성한다. 업그레이드 실패 시 기존 설치가 복원되는 순서를 검증하고, 동일 버전의 다른 ProductCode는 중복 설치를 차단한다. 다른 아키텍처·언어·설치 범위 간 전환은 별도 검증 전 지원하지 않는다.

첫 최소 MSI부터 ProductCode·UpgradeCode·PackageCode를 올바르게 발급한다. 첫 단계의 완료 조건은 파일 설치·제거이며, 중복 설치 방지의 완료 판정은 후속 업그레이드 구현 및 VM 검증 후에 한다.

근거: [Major Upgrades](https://learn.microsoft.com/en-us/windows/win32/msi/major-upgrades), [UpgradeCode](https://learn.microsoft.com/en-us/windows/win32/msi/upgradecode), [컴포넌트 코드 변경 규칙](https://learn.microsoft.com/en-us/windows/win32/msi/changing-the-component-code).
