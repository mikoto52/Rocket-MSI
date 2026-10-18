# 첫 MSI 생성기 시험

## 구현 범위

Rust CLI가 Windows Installer API로 데이터베이스와 UI 테이블을 직접 작성한다. OS의 makecab.exe로 압축한 CAB를 MSI 내부 스트림에 포함한다. 외부 크레이트, WiX, Tauri 번들러는 사용하지 않는다.

시험 제품은 `Express MSI Builder 설치 테스트` 버전 0.1.0이며 컴퓨터 전체 x64 설치다. ProductCode, UpgradeCode, Component GUID는 이 시험 제품 전용 고정값이고 PackageCode는 매번 새로 생성한다. 입력 파일 내용은 항상 설치 대상의 `hello.txt`로 패키징한다. 기본 경로는 Program Files 아래 `Express MSI Builder Demo`다.

환영 → 경로 입력 → 준비 → 진행 → 완료 화면을 구성한다. 취소·실패 화면과 설치된 제품의 제거 화면도 포함한다. 폴더 탐색 버튼, 로고, 복구 선택, 업그레이드, 사용자 제품 정보, GUID 편집 및 XML 읽기는 아직 없다.

## 생성 및 자동 검사

```powershell
cargo test --offline
cargo run --offline -- build examples/payload/hello.txt dist/emb-demo.msi
```

생성 직후 MSI를 읽기 전용으로 다시 열어 파일 행, CAB 스트림 및 화면 수를 확인한다. 통합 테스트는 한글·공백이 포함된 소스/출력 경로, 경로 확인 이벤트 순서, 언어 속성, 출력 덮어쓰기 거부를 검사한다. 이 검사는 ICE 검증 또는 실제 설치 시험을 대체하지 않는다.

2026-10-18부터 COM 초기화 후 `MsiVerifyPackageW`와 `MsiOpenPackageExW(IGNOREMACHINESTATE)` 검사도 수행한다. 후자는 시스템 상태를 변경할 수 없는 제한된 세션으로 열기만 하며 설치 액션은 실행하지 않는다. 일반 데이터베이스 읽기에 성공해도 설치 패키지 세션 열기는 실패할 수 있으므로 둘 다 확인한다.

기존 MSI를 검사하려면 다음 명령을 사용한다.

```powershell
cargo run --offline -- verify dist/emb-demo-fixed.msi
```

## Windows VM 수동 시험

스냅샷을 만든 Windows x64 시험 VM에 MSI를 복사한다. 아래 명령은 실제 설치·제거를 수행하므로 시험 VM에서만 실행한다. PowerShell의 현재 폴더는 복사한 MSI가 있는 폴더로 맞춘다.

```powershell
$msiPath = (Resolve-Path -LiteralPath .\emb-demo.msi).Path
$logPath = Join-Path (Get-Location).Path 'install.log'
msiexec.exe /i "$msiPath" /L*v "$logPath"
```

1. 한글 글꼴과 문구가 정상적으로 표시되는지 확인한다.
2. 이전/다음으로 이동하고 경로를 변경한다. 공백·한글 경로도 시험한다.
3. 설치 시작 전 취소하면 파일과 앱 등록이 남지 않아야 한다.
4. 설치 후 선택한 폴더에 hello.txt가 있고 원본 내용과 같아야 한다.
5. 설치 중 진행·완료 화면, 권한 상승과 앱 목록의 제품 등록을 확인한다.
6. MSI를 다시 열면 제거 화면이 표시되어야 한다. 제거 후 파일과 제품 등록이 사라져야 한다.

무인 설치와 제거도 별도 스냅샷에서 시험한다. PowerShell에서 종료 코드를 확인하려면 Start-Process -Wait -PassThru를 사용한다.

```powershell
$msiPath = (Resolve-Path -LiteralPath .\emb-demo.msi).Path
$logPath = Join-Path (Get-Location).Path 'silent-install.log'
msiexec.exe /i "$msiPath" /qn /L*v "$logPath"
msiexec.exe /x '{9DB58DC8-21CE-45E7-9311-8932DC040001}' /L*v .\uninstall.log
```

## 검증 상태와 한계

2026-10-17: Rust 컴파일 및 실제 MSI 생성·재열기 통합 테스트 통과. 현재 PC의 샌드박스에서 파일 접근 오류가 있어, 승인된 샌드박스 외 테스트 실행으로 확인했다. 실제 설치·제거는 수행하지 않았다. 화면의 시각 검증과 VM 수명 주기 시험은 미완료다. ICE 실행 도구가 준비되지 않아 ICE 검증도 미완료다.

프로토타입은 XML 설정이나 업그레이드 정보를 아직 작성하지 않는다. 서로 다른 제품을 만들거나 버전 업그레이드에 사용하면 안 된다. 소스는 압축 전 작업 폴더로 복사하지만, 다른 프로그램이 복사 중 변경하는 파일에 대한 일관된 스냅샷은 보장하지 않는다.

## 2026-10-18: 패키지 열기 실패 1620 수정

사용자가 제공한 dist/install.log는 RunEngine 시작 직후 MainEngineThread가 1620을 반환했다. UI/실제 설치 액션 이전의 실패였다. 기존 MSI는 테이블 조회와 MsiVerifyPackageW 검사를 통과했지만 제한된 MsiOpenPackageExW 세션 열기에서 동일한 1620을 반환했다.

요약 정보의 PID_CODEPAGE를 VT_I4/65001로 기록하던 코드를 VT_I2/1252로 수정하고 문자열 속성보다 먼저 설정했다. 요약 정보는 ASCII이며 데이터베이스의 한글 UI 문자열과는 별도다. 회귀 테스트에서 유효한 MSI를 이전 VT_I4/65001 값으로 변경하면 데이터베이스는 열리지만 세션 열기에서 1620이 재현되는 것을 확인했다.

추가로 Rust canonicalize가 만든 Windows verbatim 경로(\\?\ 접두사)는 세션 열기에서 1619가 발생하여, 세션 검사용 경로를 일반 절대 경로로 정규화했다. 이 경로 문제는 사용자가 제공한 로그의 1620 원인과는 별개다.

수정 결과물: dist/emb-demo-fixed.msi. 원본 MSI와 사용자 로그는 보존했다. 통합 테스트와 Clippy 통과. 실제 UI 표시 및 설치·제거, ICE 검증은 아직 미완료다.

시험 VM에서 수정본 실행 및 별도 로그 생성:

```powershell
$msiPath = (Resolve-Path -LiteralPath .\emb-demo-fixed.msi).Path
$logPath = Join-Path (Get-Location).Path 'install-fixed.log'
msiexec.exe /i "$msiPath" /L*v "$logPath"
```

참고: [Codepage Summary property](https://learn.microsoft.com/en-us/windows/win32/msi/codepage-summary), [MsiOpenPackageExW](https://learn.microsoft.com/en-us/windows/win32/api/msi/nf-msi-msiopenpackageexw).

## 2026-10-18: 명령행 상대 경로 오류

사용자 보고: 수정 MSI는 더블클릭으로 정상 동작하지만 안내한 명령행은 실패했다. install-fixed.log는 1620이 아닌 `Note: 1: 1324 2: . 3: 1`, 최종 반환 코드 2를 기록했다. 경로 검증에서 `.`이 거부된 것으로 보이며 `Product: .\emb-demo-fixed.msi`와 함께 상대 경로 처리 문제를 우선 의심한다. 로그만으로 내부 처리 원인 전체를 확정하지는 않는다.

시험 안내는 Resolve-Path로 MSI의 절대 경로를 계산하고 로그에도 절대 경로를 전달하도록 수정했다. 개발 PC에서 설치를 자동 실행하지 않았으므로 절대 경로 명령의 실제 설치 결과는 사용자 시험으로 확인해야 한다. 이 수정은 엔진 코드 변경이 아니다.

## 2026-10-18: 사용자 설치 성공 확인

사용자가 제공한 dist/install-absolute.log에서 수정 MSI의 절대 경로 실행을 확인했다. Windows Installer의 제품 설치 결과는 성공 상태 0이며 클라이언트와 서버 모두 MainEngineThread 반환 코드 0을 기록했다. Return value 3은 발견되지 않았다. 사용자는 더블클릭 실행도 정상 동작한다고 보고했다.

따라서 패키지 생성 → Windows Installer 실행 → 설치 성공의 첫 흐름을 사용자 시험으로 확인했다. 제거·복구·업그레이드와 파일 내용 대조, ICE 검증은 별도 완료 판정이 필요하다. 로그의 Error 테이블 조회 관련 2205/2228 메시지는 기록하되, 이번 실행은 최종 성공 상태 0이므로 설치 실패로 분류하지 않는다. 오류/진행 메시지 테이블 보강은 후속 개선 대상으로 남긴다.

## 2026-10-18: 비트맵 설치 마법사

새 시험 결과물은 dist/emb-demo-ui.msi다. 환영 화면은 왼쪽 130×235 installer units에 세로 이미지를, 나머지 7개 화면은 상단 390×52에 배너 이미지를 배치한다. 제목은 페이지마다 구분하며 배너 위에는 투명 텍스트를 사용한다. 본문은 배너 아래에서 시작하고 하단 버튼 위치는 기존과 동일하게 유지했다.

원본 비트맵은 assets/installer/welcome.bmp(260×470), banner.bmp(780×104)이며 MSI Binary 테이블에 포함한다. 원본 기하학 그림의 생성 코드는 scripts/generate-installer-art.ps1이다. 외부 이미지나 이미지 폰트가 필요하지 않다. 그림에는 문자를 넣지 않아 실제 제목과 문구는 MSI 텍스트로 유지된다.

비트맵 참조, 화면별 이미지 크기, 컨트롤 경계, 제한된 패키지 열기 및 기존 1620 회귀 검사를 통과했다. 생성한 그림은 시각적으로 확인했지만 실제 MSI 화면에서의 겹침·글꼴·DPI 배율은 아직 사용자 실행 검증이 필요하다.

시험 VM에 이전 제품이 설치되어 있으면 유지보수 화면 또는 Windows Installer의 캐시된 이전 UI가 표시될 수 있다. 새 환영 화면의 비교는 이전 시험 제품을 제거한 시험 환경이나 깨끗한 VM에서 진행한다. 이번 변경은 동일 제품·버전의 UI 시제품이며 업그레이드 배포본이 아니다.

```powershell
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\emb-demo-ui.msi" /L*v "D:\sources\Express-MSI-Builder\dist\install-ui.log"
```

실제 화면에서 확인할 항목: 환영 이미지/문구 경계, 다음 화면 상단 배너 및 제목, 100/150/200% 배율, 이전/다음/취소와 경로 변경, 진행·완료·제거 화면. MSI 비트맵 동작은 [Microsoft Bitmap Control](https://learn.microsoft.com/en-us/windows/win32/msi/bitmap-control)을 따른다.
