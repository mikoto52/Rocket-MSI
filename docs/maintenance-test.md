# 복구·제거 시험

2026-10-19 현재 화면은 [설치 UI 안내](installer-ui.md)를 따른다. 설치 복구 및 제거 페이지에서 공구·휴지통 아이콘과 설명을 표시하고 라디오 버튼으로 작업을 선택한다. 초기 선택은 없으며 선택 전에는 실행 버튼을 비활성화한다.

`실행(&E)`을 누르면 선택한 작업의 확인 페이지로 이동한다. 이 단계에서는 복구·제거를 실행하지 않는다. 확인 페이지의 `복구(&P)` 또는 `제거(&R)`를 눌러야 MSI 작업을 시작한다. `< 뒤로(&B)`는 선택 화면으로 돌아가고 `취소(&C)`는 작업명에 맞는 취소 확인을 표시한다. 약관은 유지보수에서 다시 요구하지 않는다.

시험 항목: 선택 전 실행 비활성화, 두 작업의 확인 문구, 뒤로 이동 후 선택 변경, 각 확인 페이지에서 취소의 예/아니요, 실제 복구·제거 실행, 작업별 진행·완료 문구. 설명과 라디오 그룹이 겹치지 않고 제목·단축키·포커스가 잘리지 않는지도 확인한다.

## 초기 엔진 시험 이력

아래는 2026-10-18 초기 프로토타입의 정책·결과물·시험 절차 기록이다. 화면과 리소스 지원 상태는 이후 변경되었으며 현재 동작은 위 안내 및 파일 정책·레지스트리·바로가기 시험 문서를 따른다.

## 복구 정책

복구 버튼은 REMOVE 속성 초기화 → ReinstallMode(amus) → Reinstall(ALL) → EndDialog(Return) 순서로 표준 MSI 이벤트를 발행한다. 파일이 없거나 변경되어 있어도 패키지 원본으로 다시 복사하는 정책이다. 향후 사용자 편집 데이터는 설치 파일과 분리해야 하며, 현재 패키지에는 시험용 hello.txt만 있다. `m/u/s`는 MSI가 관리하는 컴퓨터/사용자 레지스트리 및 바로가기 복구 모드다. 현재 프로토타입에 레지스트리·바로가기 리소스는 아직 없으므로 해당 리소스 복구를 검증했다고 간주하지 않는다.

제거 버튼은 REINSTALL 속성을 지운 다음 REMOVE=ALL을 설정하고 기존 실행 시퀀스를 진행한다. 완료·진행 문구는 설치와 유지보수에서 공용으로 사용한다. 약관은 Installed 조건에 따라 복구·제거 때 다시 요구하지 않는다.

참고: [Reinstall 이벤트](https://learn.microsoft.com/en-us/windows/win32/msi/reinstall-controlevent), [ReinstallMode 이벤트](https://learn.microsoft.com/en-us/windows/win32/msi/reinstallmode-controlevent), [복구 모드](https://learn.microsoft.com/en-us/windows/win32/msi/reinstallmode).

## 시험 순서

이번 시제품도 같은 제품 코드와 버전을 사용한다. 새 UI를 확인하려면 이전 시험 제품을 먼저 제거한 환경에서 새 패키지를 설치한다. 개발 PC에서 자동 설치·제거하지 않는다.

1. 새 MSI를 설치하고 선택한 설치 폴더의 hello.txt 내용을 확인한다.
2. 설치에 사용한 동일 MSI를 다시 열어 복구·제거 버튼이 보이는지 확인한다.
3. 시험용 hello.txt만 삭제한다. 복구 실행 후 파일이 다시 생성되고 원본과 같은지 확인한다.
4. hello.txt 내용을 변경하고 다시 복구해 원본으로 돌아오는지 확인한다.
5. 사용자 지정 설치 경로에서도 같은 위치에 복원되는지 확인한다.
6. 취소 시 파일 변경이 없는지 확인한다.
7. 제거 실행 후 파일과 설치된 앱 등록이 사라지는지 확인한다.

PowerShell 실행 예시(실제 설치 작업이므로 시험 환경에서 실행):

```powershell
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\emb-demo-maintenance.msi" /L*v "D:\sources\Express-MSI-Builder\dist\maintenance.log"
```

무인 복구는 설치 후 해당 MSI의 절대 경로로 실행한다.

```powershell
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\emb-demo-maintenance.msi" /qn REINSTALL=ALL REINSTALLMODE=amus /L*v "D:\sources\Express-MSI-Builder\dist\repair-silent.log"
```

원본 MSI가 필요할 수 있으므로 시험 중 삭제하지 않는다. 실제 복구 시험 시에는 MSI 로그의 최종 성공 상태와 파일 내용까지 확인한다.

## 검증 상태

공통 UI 간격 조정: 버튼 크기를 72×20에서 66×18 installer units로 줄였다. 이전 X=174, 다음/진행 X=240으로 맞붙이고 취소 X=314로 8단위 간격을 둔다. 버튼 Y=254이며 창 오른쪽 여백은 10, 아래 여백은 8이다. 상단 배너 높이는 60에서 52로 줄이고 제목/부제도 위로 조정했다. 최신 간격 시험본: `dist/emb-demo-ui-bottom-right.msi`.

사용자 시험 결과: 이전 `emb-demo-maintenance.msi`에서 복구·제거가 모두 정상 작동함을 2026-10-18 보고받았다. 이후 라디오 선택 방식으로 UI를 변경했다. 최신본은 선택별 이벤트 조건·순서, 기본값, 라디오 두 항목 및 진행 버튼 문구를 자동 검사했다. 라디오 UI에서의 실제 클릭 동작은 재확인 대상이다.

자동 검사: MSI 생성 및 제한된 세션 열기, 복구/제거 이벤트의 인수와 순서, 유지보수 버튼 수, 기존 약관·브랜딩·1620 회귀 검사. 정적 검사 통과. 실제 파일 삭제 후 복구 및 제거는 사용자 시험 전이므로 미검증이다.
