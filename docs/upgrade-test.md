# 버전 업그레이드 첫 구현

`.embproj`에 `<Upgrade enabled="true" />`를 넣는다. 새 프로젝트는 기본으로 활성화하고, 이 요소가 없는 기존 프로젝트는 기존 동작을 유지한다. UpgradeCode는 유지하며 ProductVersion을 올리고 ProductCode를 새로 발급한다. 같은 MSI 재실행은 기존 복구·제거 화면을 사용한다.

구현은 MSI 표준 FindRelatedProducts/Upgrade 테이블과 트랜잭션 안의 InstallExecute → RemoveExistingProducts → InstallFinalize 순서다. 먼저 새 버전을 설치하고, 구버전에서만 사용하는 파일·레지스트리·바로가기를 정리한다. 동일 리소스의 컴포넌트 GUID는 유지하며 기존 파일 처리 정책(default/skip/overwrite)을 적용한다. 새 파일이 반드시 교체돼야 한다면 overwrite를 선택하고 실제 제품 파일로 확인한다. 사용자가 별도로 만든 파일은 제거 대상으로 선언하지 않는 한 유지한다.

상위 버전이 설치돼 있으면 다운그레이드를 차단하고, 같은 버전·다른 ProductCode도 중복 설치를 차단한다. 두 검사 모두 UI/실행 시퀀스에 있어 무인 설치에도 적용된다. 자동 실행 목록은 업그레이드 중에는 실행하지 않는다.

## 현재 제한

- 첫 버전부터 Upgrade를 활성화한 동일 아키텍처·동일 설치 위치 구성의 패키지를 대상으로 한다.
- 최초 설치 경로를 HKLM의 `Software\InstallEmbly\Packages\<UpgradeCode 중괄호 제외>\<architecture>`에 저장하고 업그레이드 때 복원한다.
- 아키텍처·설치 경로 템플릿 결과가 달라졌거나 기존 패키지에 내부 업그레이드 정보가 없으면 자동 교체를 차단한다. 설치 위치를 바꾸려면 제거 후 새로 설치한다.
- 업그레이드 중 설치 경로 변경도 차단한다. ProductVersion을 설치 폴더 이름에 포함하면 버전마다 구성이 달라지므로 이 첫 구현의 자동 업그레이드 대상이 아니다.
- 프로그램 이름·제작사를 바꿔 기본 설치 경로가 달라지는 경우에도 같은 제한이 적용된다. 임의의 컴포넌트 구성 변경·다른 설치 도구 제품의 이전은 지원하지 않는다.
- 관리자 실행 비교가 필요했던 롤백 정리 권한 문제는 별도 미해결 항목이다. 부트스트래퍼는 후속 작업이다.

## 시험 키트

`scripts/create-upgrade-demo.ps1`은 `dist/upgrade-demo`에 독립 ProductCode와 공통 UpgradeCode를 가진 v1, v2, v2-duplicate, v3-fail을 만든다. 기본 경로는 `C:\Program Files\InstallEmbly\Upgrade Demo`이며 기존 파일 정책은 overwrite다. v3-fail은 구버전 제거 직후 강제로 실패시켜 전체 트랜잭션의 롤백을 확인하는 시험 전용 MSI다. 실제 설치를 자동 실행하지 않는다.

관리자 PowerShell에서 아래 순서로 시험한다. 각 작업이 끝난 뒤 검사 명령을 실행한다.

```powershell
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\upgrade-demo\v1\Setup.msi" /L*V "D:\sources\Express-MSI-Builder\dist\upgrade-demo\v1\install.log"
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-upgrade-demo.ps1" -Phase V1
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\upgrade-demo\v2\Setup.msi" /L*V "D:\sources\Express-MSI-Builder\dist\upgrade-demo\v2\install.log"
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-upgrade-demo.ps1" -Phase V2
```

설치된 제품이 하나이고 version.txt/레지스트리가 2.0.0이며, obsolete.txt는 없어지고 new-file.txt가 생겨야 한다. 시작 메뉴 바로가기도 유지돼야 한다. v1 설치 위치를 변경했다면 모든 검사에 -InstallDir을 추가해 기존 경로 승계도 확인한다.

다음으로 v1을 다시 실행하면 다운그레이드 차단, v2-duplicate를 실행하면 동일 버전 중복 설치 차단이 나와야 한다. 각 작업 후 `-Phase Blocked`로 v2 유지 상태를 검사한다. v3-fail 설치는 오류로 끝나야 하며 `-Phase RolledBack`으로 v2 파일·레지스트리·제품 등록·바로가기 복원을 검사한다. 마지막에 v2를 제거하고 `-Phase Removed`로 확인한다.

자동 테스트는 버전 범위·충돌 조건·시퀀스·공유 컴포넌트 GUID·실제 MSI 세션 열기와 AppSearch 실행까지 확인한다. 검색 테스트는 설치 시퀀스를 실행하지 않는다. 첫 v1 수동 시험에서 발견한 Signature 테이블 누락을 수정했다.

2026-10-18 사용자 수동 시험 결과: 아래 여섯 단계 모두 PASS를 확인했다.

- V1: 최초 설치 후 제품 등록 하나, 파일·레지스트리·바로가기 확인.
- V2: 업그레이드 후 제품 등록 하나, 새 파일·레지스트리·바로가기 및 구버전 전용 파일 제거 확인.
- Blocked: v1 다운그레이드 차단 후 v2 유지 확인.
- Blocked: v2-duplicate 동일 버전 중복 설치 차단 후 v2 유지 확인.
- RolledBack: v3-fail 의도적 실패 후 v2 파일·레지스트리·제품 등록·바로가기 복원 확인.
- Removed: 업그레이드된 v2 제거 검사 통과.

설치 경로 변경 후 승계와 무인 업그레이드는 이번 보고에서 별도로 검증하지 않았다.

참고: [Upgrade 테이블](https://learn.microsoft.com/en-us/windows/win32/msi/upgrade-table), [RemoveExistingProducts](https://learn.microsoft.com/en-us/windows/win32/msi/removeexistingproducts-action).
