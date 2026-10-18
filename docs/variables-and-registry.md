# 변수 및 레지스트리 명세

현재 구현 범위는 [레지스트리 첫 구현 및 시험](registry-test.md)을 기준으로 한다. REG_SZ·REG_DWORD·REG_BINARY·REG_MULTI_SZ 및 기본 경로 변수를 지원한다. 아래 내용에는 설치일·드라이브·사용자 변수·keep 등 후속 설계가 포함된다. 현재 경로 변수는 단독 사용 시 MSI의 끝 역슬래시를 유지한다.

## 1. 공통 문법

변수 참조는 `{Name}`이며 이름은 대소문자를 구분한다. 사용자 변수 이름은 `[A-Za-z][A-Za-z0-9_]*`로 제한하고 기본 변수 이름을 재정의할 수 없다. 중괄호 자체는 `{{`, `}}`로 표현한다. 파서는 이를 리터럴로 보존하며 치환 후 결과를 다시 변수 구문으로 해석하지 않는다.

사용자 변수는 문자열 템플릿이며 다른 변수를 참조할 수 있다. 의존 그래프를 검사해 순환 참조와 미정의 참조를 빌드 전에 차단한다. MSI 속성 구문인 `[PROPERTY]`는 사용자 문법으로 노출하지 않으며 일반 텍스트로 이스케이프한다.

허용 위치는 레지스트리 키 경로·값 이름·문자열 데이터, 사용자 변수 값, 기본 설치 경로 템플릿이다. DWORD 데이터는 상수만 허용한다. 다중 문자열은 각 원소에 변수를 허용한다. 소스 파일 경로에는 설치 시점 변수를 허용하지 않는다.

설치·제거 실행 항목의 대상 PC 프로그램 경로, 인수 및 작업 폴더에도 변수를 허용한다. 제거 시에는 기존 설치 상태를 기준으로 평가한다. 세부 사항은 [프로그램 실행 명세](program-actions.md)를 따른다.

## 2. 기본 변수

| 이름 | 의미 | 평가/구현 방향 |
|---|---|---|
| ProductName | 제품 이름 | 프로젝트 상수 |
| ProductVersion | 제품 버전 | 프로젝트 상수 |
| Manufacturer | 제작사 | 프로젝트 상수 |
| InstallDir | 최종 설치 폴더 | 설치 대상의 확정된 INSTALLDIR |
| InstallDate | 해당 버전의 최초 성공 설치일 | 대상 PC 로컬 날짜, `yyyy-MM-dd`; 별도 구현 검증 필요 |
| ProgramFiles | 패키지 아키텍처에 대응하는 폴더 | x86: ProgramFilesFolder, x64: ProgramFiles64Folder |
| WindowsDir | Windows 폴더 | WindowsFolder |
| ProgramData | 모든 사용자 앱 데이터 폴더 | CommonAppDataFolder |
| SystemDrive | 대상 PC의 SystemDrive 환경 값 | 설치 실행 컨텍스트에서 읽고 드라이브 형식 검증 |
| InstallDrive | 최종 InstallDir의 드라이브 | 디렉터리 확정 후 추출 |
| WindowsDrive | Windows 폴더의 드라이브 | WindowsVolume 기반으로 정규화 |

표의 MSI 속성 연결은 설계 방향이다. 설치 컨텍스트에서의 실제 값과 평가 순서는 기술 검증 대상으로 둔다. SystemDrive와 WindowsDrive는 일반적으로 같지만 서로의 대체값으로 구현하지 않는다. 부팅 관리자 파티션을 찾는 변수도 아니다.

## 3. 경로 규칙

- 디렉터리 변수는 끝의 `\`를 제거하되 루트 `C:\`는 유지한다. 드라이브 변수는 `C:`처럼 끝의 `\` 없이 반환한다.
- 경로 조합은 `{InstallDrive}\Company\Data`처럼 작성한다. 디렉터리 경계의 중복 구분자는 경로 전용 처리에서만 정규화한다. 임의 레지스트리 문자열은 변경하지 않는다.
- 첫 버전 설치 대상은 로컬 드라이브의 절대 경로로 제한한다. 상대 경로, `C:Apps` 같은 드라이브 상대 경로, UNC 경로는 설치 단계에서 거부한다.
- 기본 설치 경로에는 InstallDir/InstallDrive와 이를 간접 참조하는 사용자 변수를 금지한다. 최종 설치 경로를 결정하는 과정의 순환을 방지한다.
- 예시의 C:/D: 값은 설명용이며 빌드 결과에 상수로 삽입하지 않는다.

## 4. 평가 시점과 수명 주기

빌드 시 상수 및 참조를 검증하고 설치 시 필요한 표현식을 MSI 속성 및 필요한 보조 동작으로 변환한다. 설치 UI에 의존하지 않고 실행 시퀀스에서도 평가되어야 한다. InstallDrive는 디렉터리 결정 이후, 레지스트리 쓰기 이전에 확정한다. 상승 권한 실행 경계를 넘는 속성 전달도 검증한다.

설치일자의 초기 정책 제안:

| 상황 | InstallDate 정책 |
|---|---|
| 신규 설치 | 설치 트랜잭션 시작 시 대상 PC 로컬 날짜를 계산하고 성공 시 남김 |
| 복구/동일 버전 재설치 | 저장된 날짜 보존; 날짜 저장값이 없는 기존 설치는 복구 수행일로 보완 |
| 상위 버전 설치 | 새 버전 설치일로 갱신 |
| 실패 및 롤백 | 기존 값 복원, 신규 값은 남기지 않음 |
| 제거 후 재설치 | 새 설치일 부여; 유지된 사용자 항목의 오래된 날짜를 그대로 재사용하지 않음 |

이는 제품 전체의 최초 설치일과 다르다. 별도의 최초 설치일 이력은 후속 기능으로 둔다. MSI Date 속성을 단순 치환해서 고정 형식과 보존 동작이 보장된다고 가정하지 않는다. 내부 상태와 사용자가 편집하는 레지스트리 항목을 분리한다.

## 5. 레지스트리 항목

| 필드 | 규칙 |
|---|---|
| id | 프로젝트 내 안정적인 식별자 |
| root | HKLM 또는 HKCU |
| key | 루트 이하의 키 경로 템플릿; 빈 경로 금지 |
| name | 값 이름 템플릿; 빈 문자열은 기본값 |
| type | REG_SZ, REG_DWORD, REG_BINARY, REG_MULTI_SZ; REG_EXPAND_SZ는 후속 후보 |
| value | 문자열 / DWORD 정수 / HEX 바이트 / 문자열 배열 |
| view | registry32 또는 registry64; x86 패키지의 registry64는 차단 |
| uninstall | remove(기본) 또는 keep |

DWORD는 0–4294967295 범위의 정수이며 UI에서 10진수·16진수 입력을 지원하되 저장 시 정수로 정규화한다. 다중 문자열은 줄 목록 UI와 문자열 배열로 표현하며 공백 원소·빈 원소 처리는 MSI 인코딩 검증 후 확정한다. REG_EXPAND_SZ에서 `{ProgramData}`는 설치 때 값으로 치환하고 `%ProgramData%`는 원문을 저장해 앱/Windows가 나중에 확장하도록 구분한다.

중복 판정은 루트, 영역, 키, 값 이름을 기준으로 대소문자를 무시한다. 동적 참조의 실제 충돌을 빌드 때 판단할 수 없으면 경고하고, 설치 시 평가 후 충돌은 변경 전에 차단한다. 변수 결과에 키 구분자나 금지 문자가 생기는지도 위치에 맞게 검증한다.

keep는 제거 후에도 값을 남기는 의도적인 옵션이다. 해당 항목을 별도 컴포넌트로 분리하고 업그레이드·복구 동작을 시험한다. 키 전체의 재귀 삭제는 제공하지 않는다. 기존 값 덮어쓰기와 제거는 전용 제품 키 아래에서 사용하는 것을 기본으로 안내하고 기존 값 복원 여부를 MSI 트랜잭션 시험에서 확인한다.

## 6. 예시

```text
HKLM\Software\MyCompany\MyApp   (registry64)
  InstallPath    REG_SZ         {InstallDir}
  InstallDate    REG_SZ         {InstallDate}
  ProgramFiles   REG_SZ         {ProgramFiles}
  DataPath       REG_SZ         {ProgramData}\MyCompany\MyApp
  SystemDrive    REG_SZ         {SystemDrive}
  InstallDrive   REG_SZ         {InstallDrive}
  WindowsDir     REG_SZ         {WindowsDir}
  WindowsDrive   REG_SZ         {WindowsDrive}
```

위 설정은 경로를 레지스트리에 기록한다. DataPath의 실제 폴더 생성이나 쓰기 권한 부여를 자동으로 의미하지 않는다.

## 공식 참고 자료

- [MSI 속성 목록](https://learn.microsoft.com/en-us/windows/win32/msi/property-reference): 대상 폴더 속성과 날짜 속성 확인.
- [Formatted](https://learn.microsoft.com/en-us/windows/win32/msi/formatted): MSI 속성 및 특수 문자 처리. 빌더 문법과 별도 계층이다.
- [Registry Table](https://learn.microsoft.com/en-us/windows/win32/msi/registry-table): 레지스트리 데이터의 MSI 인코딩.
- [Installation Context](https://learn.microsoft.com/en-us/windows/win32/msi/installation-context): 사용자별/컴퓨터 전체 설치 차이.
