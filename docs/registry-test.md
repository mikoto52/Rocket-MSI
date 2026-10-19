# 레지스트리 첫 구현 및 시험 키트

현재 구현은 REG_SZ, REG_DWORD, REG_BINARY, REG_MULTI_SZ 네 형식이다. .embproj의 선택적 Registry 요소 아래에 Entry를 반복한다. HKLM/HKCU, 아키텍처 자동 영역 또는 명시적 registry32/registry64를 지원한다. x86 패키지에서 registry64는 거부한다. 기본 제거 정책은 remove이며 현재 keep와 키 전체 재귀 삭제는 지원하지 않는다.

```xml
<Registry>
  <Entry id="path" root="HKLM" key="Software\{Manufacturer}\MyApp"
         name="InstallPath" type="REG_SZ" view="auto" uninstall="remove">
    <Data>{InstallDestination}</Data>
  </Entry>
  <Entry id="flags" root="HKLM" key="Software\MyCompany\MyApp" name="Flags" type="REG_DWORD"><Data>0xFFFFFFFF</Data></Entry>
  <Entry id="blob" root="HKLM" key="Software\MyCompany\MyApp" name="Blob" type="REG_BINARY"><Data>00 01 7F 80 FF</Data></Entry>
  <Entry id="paths" root="HKLM" key="Software\MyCompany\MyApp" name="Paths" type="REG_MULTI_SZ">
    <Data><String>{InstallDestination}\hello.txt</String><String>  공백 유지  </String></Data>
  </Entry>
</Registry>
```

id는 영문·숫자·밑줄이며 숫자로 시작할 수 없고 최대 64자다. name=""는 키의 기본값이다. DWORD는 0..4294967295의 10진수 또는 소문자 0x 접두사 16진수다. BINARY는 공백을 제외한 짝수 길이 HEX이며 비어 있는 바이너리는 현재 거부한다. MULTI_SZ는 한 개 이상의 비어 있지 않은 String이 필요하다. 문자열 순서·앞뒤 공백을 보존하며 기존 목록에 추가하지 않고 교체한다. 문자열 안의 NUL은 허용하지 않는다.

REG_SZ와 MULTI_SZ 항목은 `{InstallDestination}`, `{ProgramFiles}`, `{ProgramData}`, `{WindowsDir}`, `{ProductName}`, `{Manufacturer}`, `{ProductVersion}`을 지원한다. 키·값 이름은 제품명·제작사·버전 상수만 지원한다. `{{`, `}}`는 리터럴 중괄호이며 `[PROPERTY]`, `[~]`, 선두 # 등은 문자열로 보존한다. DWORD/BINARY는 상수만 지원한다. 알 수 없는 변수·중복 목적지·잘못된 데이터는 빌드 전 거부한다.

현재 경로 변수는 MSI 디렉터리 속성을 사용하므로 **단독 사용 시 마지막에 역슬래시가 남는다.** `{InstallDestination}\hello.txt`처럼 뒤에 역슬래시를 쓰면 하나를 합쳐 중복을 피한다. 드라이브·설치일·사용자 변수는 후속 구현이다. 기존 변수 문서의 끝 구분자 제거 규칙은 목표 설계이며 현재 경로 변수 동작과 구분한다.

각 값은 독립 RegistryKeyPath 컴포넌트로 만들어 설치·복구·제거 표준 액션을 사용한다. 제거는 선언한 값만 대상으로 하며 관련 없는 값은 지우지 않는다. 설치 전 같은 이름의 값은 덮어쓸 수 있고, 성공 설치 후 제거할 때 이전 값을 되살리는 기능은 아니다. HKCU는 설치 실행 사용자 컨텍스트에 의존하며 별도 관리자 자격증명을 쓰는 환경은 추가 검증이 필요하다.

## 설치 시험

이미 생성한 패키지는 `dist/registry-demo/Setup.msi`다. 다시 생성하려면 출력 폴더를 새로 지정하고 다음 명령을 실행한다.

```powershell
cargo run --offline -- build examples/RegistryDemo.embproj
```

시스템 레지스트리를 수정하는 시험이므로 전용 시험 환경에서 진행한다. 현재 키트는 전용 `HKLM\Software\InstallEmbly\RegistryDemo` 아래 64비트·32비트 영역을 사용한다. 기존 제품 데이터가 있는 키에 적용하지 않는다.

1. MSI를 설치한다. 기본 설치 위치는 `C:\Program Files\InstallEmbly\Registry Demo`다.
2. 다음 명령으로 네 형식·변수·공백·한글·DWORD 최댓값·기본값·32비트 영역을 확인한다.

```powershell
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-registry-demo.ps1" -Phase Installed
```

설치 위치를 바꿨다면 검사 스크립트에 `-InstallDir "D:\선택한 폴더"`를 모든 단계에 추가한다.

## 복구 및 제거 시험

관리자 PowerShell에서 다음 명령으로 형식별 값 네 개를 지우고 관련 없는 대조 값을 추가한다. 현재 등록 값이 예제와 일치하는지 확인한 후에만 변경한다.

```powershell
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-registry-demo.ps1" -Phase PrepareRepair
```

같은 MSI를 열어 복구한다. 일반 실행 복구에서 롤백 정리 권한 오류가 나면 이전 시험에서 확인한 관리자 실행 경로로 재시험하며 성공으로 간주하지 않는다. 관리자 PowerShell의 다음 명령은 로그를 남긴다.

```powershell
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\registry-demo\Setup.msi" /L*V "D:\sources\Express-MSI-Builder\dist\registry-demo\repair.log"
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-registry-demo.ps1" -Phase Repaired
```

복구가 완료된 뒤 검사 명령을 실행한다. 마지막으로 같은 MSI에서 제거하고 다음을 실행한다.

```powershell
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-registry-demo.ps1" -Phase Removed
```

선언한 값은 모두 없어지고 Unrelated 대조 값은 남아야 한다. 따라서 시험 키가 남아 있어도 정상이다. 자동 테스트는 MSI 데이터베이스와 설치 세션 열기까지만 수행하며 실제 레지스트리를 변경하지 않는다.

참고: [Registry 테이블](https://learn.microsoft.com/en-us/windows/win32/msi/registry-table), [WriteRegistryValues](https://learn.microsoft.com/en-us/windows/win32/msi/writeregistryvalues-action), [RemoveRegistryValues](https://learn.microsoft.com/en-us/windows/win32/msi/removeregistryvalues-action).
