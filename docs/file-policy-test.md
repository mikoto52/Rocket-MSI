# 기존 파일 처리 정책 및 수동 시험

프로젝트 전체 기본 정책을 Installation.existingFiles로 지정한다. 파일별 예외와 Tauri GUI는 후속 구현이다.

```xml
<Installation directory="{ProgramFiles}\{Manufacturer}\{ProductName}"
              allowDirectoryChange="true" existingFiles="skip" />
```

| 값 | 설치 및 마법사 복구 | MSI REINSTALLMODE |
|---|---|---|
| default (생략 시 기본) | MSI 기본 버전·수정 여부 규칙에 따라 교체 | omus |
| skip | 파일이 있으면 유지하고, 없으면 설치·복구 | pmus |
| overwrite | 파일을 패키지 내용으로 교체 | amus |

EXE/DLL의 실제 버전·언어 리소스를 File 테이블에 기록한다. 버전 없는 파일은 해당 필드를 비운다. `default`는 단순 날짜 비교가 아니며 MSI 규칙을 따른다. 프로젝트 기반 마법사 복구에도 선택한 정책이 적용되므로 default/skip은 수정 파일을 강제 초기화하지 않는다. 이전 소스·출력 직접 지정 CLI의 복구는 기존 강제 복구 동작을 유지한다.

명령줄에서 REINSTALLMODE나 `/f` 옵션을 직접 지정하면 정책을 바꿀 수 있다. 관리 설치(`/a`)는 이 정책의 시험 대상이 아니다. Windows에서 보호하는 리소스 등에 대한 덮어쓰기 예외도 존재한다.

**skip은 설치·복구 시의 복사 정책이다. 제거 시 파일 보존 옵션이 아니다.** 기존에 있었던 파일도 제품의 제거 대상이 될 수 있다. 사용자 설정·DB의 제거 시 보존은 별도 기능으로 구현한다. 이번 시험은 전용 폴더의 가짜 데이터만 사용한다. 버전 간 자동 업그레이드는 아직 지원하지 않는다.

## 키트 생성

```powershell
powershell -NoProfile -File scripts/create-file-policy-demo.ps1
```

dist/file-policy-demo 아래 정책별 .embproj와 embedded CAB 방식의 Setup.msi 세 개를 만든다. 모두 독립된 ProductCode·UpgradeCode를 사용한다. 기존 키트는 덮어쓰지 않는다. 스크립트는 MSI를 설치하지 않는다.

## 1. skip 시험

저장소 루트에서 실행한다.

```powershell
powershell -NoProfile -File scripts/test-file-policy-demo.ps1 -Policy skip -Phase Prepare
```

Prepare는 dist/file-policy-demo/targets/skip에 existing.txt를 만들고, 실행할 msiexec 명령을 출력한다. **출력된 명령을 실행**해서 해당 폴더에 설치한다. 다른 설치 경로를 선택하면 검사가 맞지 않는다.

```powershell
powershell -NoProfile -File scripts/test-file-policy-demo.ps1 -Policy skip -Phase Installed
powershell -NoProfile -File scripts/test-file-policy-demo.ps1 -Policy skip -Phase PrepareRepair
```

첫 검사는 existing.txt의 기존 내용 유지와 missing.txt 신규 설치를 확인한다. PrepareRepair는 existing.txt를 수정하고 missing.txt만 삭제한다. 그다음 같은 MSI를 열어 **복구**를 선택한다.

```powershell
powershell -NoProfile -File scripts/test-file-policy-demo.ps1 -Policy skip -Phase Repaired
```

수정 파일 유지와 누락 파일 복원을 확인한다. 같은 MSI를 열어 **제거**한 뒤 다음 명령으로 확인한다.

```powershell
powershell -NoProfile -File scripts/test-file-policy-demo.ps1 -Policy skip -Phase Removed
```

시험 마커 파일과 로그는 남긴다. 준비 단계는 기존 시험 폴더를 덮어쓰지 않는다.

## 2. overwrite 및 default

위 명령의 `-Policy skip`을 `overwrite`, `default`로 바꿔 동일하게 진행한다. 각 Prepare가 출력한 해당 정책의 MSI 명령을 사용한다.

| 검사 | skip | overwrite | default |
|---|---|---|---|
| 설치 후 existing.txt | PREEXISTING USER CONTENT | PACKAGE CONTENT | PREEXISTING USER CONTENT |
| 복구 후 existing.txt | USER MODIFIED AFTER INSTALL | PACKAGE CONTENT | USER MODIFIED AFTER INSTALL |
| 누락 파일 | 설치·복구됨 | 설치·복구됨 | 설치·복구됨 |

default 시험 파일은 버전이 없고 생성 시각과 수정 시각이 다르도록 준비한다. 다른 EXE/DLL이나 날짜 상태의 파일에서는 기본 MSI 규칙에 따라 결과가 달라질 수 있다.

## 참고

- [REINSTALLMODE](https://learn.microsoft.com/en-us/windows/win32/msi/reinstallmode)
- [기존 파일 교체 규칙](https://learn.microsoft.com/en-us/windows/win32/msi/replacing-existing-files)
- [파일 버전 읽기 API](https://learn.microsoft.com/en-us/windows/win32/api/msi/nf-msi-msigetfileversionw)
