# Quick MSI 첫 단위

2026-10-19 구현. Windows 10/11 대상의 별도 Rust UI DLL을 MSI의 `MsiEmbeddedUI` 테이블에 포함한다. MSI 파일 하나로 배포하며 WebView2나 별도 Setup.exe를 요구하지 않는다. x86/x64 패키지에는 해당 아키텍처 DLL을 넣는다. 기존 마법사는 기본값으로 유지한다.

## 설정

빌더의 설치 화면 → 설치 화면 방식에서 Quick MSI를 선택하거나 프로젝트에 다음과 같이 지정한다. mode 생략은 wizard이며 알 수 없는 값은 거부한다.

```xml
<UI mode="quick">
  <BrandingText>Rocket MSI</BrandingText>
  <FinishLaunch file="DemoLaunch.exe">
    <Arguments>
      <Argument>first argument</Argument>
      <Argument>[literal]</Argument>
    </Arguments>
  </FinishLaunch>
</UI>
```

`FinishLaunch`는 선택 사항이다. 대상은 설치 파일 목록에 포함된 EXE여야 한다. 설치 경로는 프로젝트의 기본 경로 또는 MSI 속성 설정을 사용한다. 첫 단위 UI에는 경로 편집·제품 아이콘 편집·사용권 계약 표시가 없다. License를 함께 지정하면 검증·빌드를 차단한다. 파일·레지스트리·바로가기·업그레이드 등은 기존 MSI 엔진을 공유한다.

## 화면과 동작

- 준비: `{ProductName} 설치`, `Publisher: {Manufacturer}`, `Version: {ProductVersion}`, 오른쪽 큐브 아이콘과 설치 버튼. 신규 설치 첫 화면에는 취소 버튼을 표시하지 않으며 창 닫기·Esc로 취소할 수 있다.
- 설치 후 실행이 지정되면 왼쪽 아래에 체크박스를 표시한다. 기본값은 해제이며 설치 시작 전에 선택한다.
- 진행: 같은 창에서 작업 설명과 진행 막대를 표시하고 취소 버튼을 제공한다. 취소는 확인 후 MSI에 요청하며 실제 중단과 롤백은 MSI 상태에 따른다.
- 체크한 신규 설치가 성공하면 완료 화면을 표시하지 않고 설치된 EXE를 실행한 뒤 닫는다. 실패하면 창을 유지하고 오류를 표시한다.
- 체크하지 않았으면 `{ProductName} 설치 완료`와 제품 정보를 유지하고 실행/완료 버튼을 제공한다. 실행 대상이 없으면 완료 버튼만 제공한다.
- 유지보수: 제목은 `{ProductName} 설치`, 동일한 Publisher·Version·아이콘을 사용하며 오른쪽 아래에 삭제/복구/취소 버튼을 배치한다. 삭제는 MSI REMOVE=ALL, 복구는 REINSTALL=ALL과 기존 REINSTALLMODE 정책을 사용한다.

버튼은 작은 라운드를 적용하고 설치 후 실행 체크박스는 같은 색상의 사각형과 체크 표시로 직접 그린다. 체크박스는 네이티브 컨트롤의 선택 상태·키보드·접근성 동작을 유지하며 그리기만 대체한다.

첫 설치·유지보수 화면은 설치 시작이나 작업 선택 안내 문구 없이 표시한다. 마지막 화면도 정상 설치·복구·삭제 및 사용자 취소 시 본문 안내를 생략한다. 오류나 설치 결과 확인 불가, 설치 후 실행 실패가 발생한 경우에만 본문에 오류·결과 확인 메시지를 표시한다.

창은 기본 제목 표시줄을 숨긴 Borderless 방식이다. 상단의 브랜드 제목은 표시하지 않으며 하단 브랜딩은 유지한다. 상단을 끌어 이동하며 오른쪽의 직접 그린 최소화·닫기 버튼, Esc·Alt+F4를 제공한다. 최소화 버튼에 마우스를 올리면 회색, 닫기 버튼은 빨간색 배경으로 표시한다. Windows 기본 창 스타일과 DWM 프레임 확장을 사용해 Aero 그림자를 유지한다. 창은 760×480 논리 픽셀의 고정 크기로 표시하고 DPI에 맞춰 배율을 적용한다. 가장자리 드래그·시스템 메뉴 크기 조절·최대화·상단 더블클릭 확대는 차단하며 이동과 최소화는 유지한다. 둥근 모서리는 지원하는 Windows에서만 적용하며 Windows 10의 실제 효과는 별도 확인이 필요하다. 제품명 아래에 Publisher·Version을 가까이 배치한다. 타이머는 상태가 바뀐 경우에만 갱신하며, 버튼 배치는 단계 전환·크기 변경 때만 계산한다. 본문은 메모리 비트맵에서 그린 뒤 한 번에 복사하여 배경 지우기와 반복 배치로 인한 깜빡임을 줄인다.

Windows의 앱 색상 설정(AppsUseLightTheme)을 읽어 밝은 모드와 어두운 모드를 자동 선택한다. 어두운 모드에서는 검정 배경과 흰 글씨, 어두운 보조 버튼·체크박스·진행 막대를 사용한다. 설정 변경 알림을 받으면 열린 창에도 새 테마를 반영한다. 설정을 읽을 수 없으면 밝은 모드로 표시한다. Windows 설정 자체는 변경하지 않는다.

INSTALLEND 콜백의 결과는 프로세스 종료 코드와 구별한다. 관찰된 성공값 1과 정상 완료값 5는 내부 성공 코드 0으로, 취소 2는 1602, 실패 3은 1603으로 변환한다. 미실행·중단 등은 성공으로 취급하지 않는다. [MSI 작업 결과값 문서](https://learn.microsoft.com/en-us/windows/win32/msi/logging-of-action-return-values)를 참고한다. 이 구분과 실행 가능 여부를 회귀 검사한다.

설치 후 실행은 실패·취소·복구·제거·관련 제품이 있는 업그레이드에서 제공하지 않는다. 실행은 MSI 세션 핸들을 다른 스레드에서 재사용하지 않고, 성공 후 제품 코드와 컴포넌트 GUID로 등록된 설치 경로를 조회하여 수행한다. 인수는 각각의 원문 문자열을 유지한다.

`/passive`와 `/quiet`는 Embedded UI를 표시하지 않는다. DLL의 Attributes는 1로 두어 Basic UI 처리를 가로채지 않는다. 내부 UI를 직접 대체하는 전체 UI 설치에만 적용한다. MSI 로그와 종료 코드는 기존 msiexec 방식으로 확인한다.

## 시험 키트

```powershell
powershell -NoProfile -File scripts/create-quick-demo.ps1
# dist/quick-demo/package/Setup.msi 생성. 설치는 실행하지 않는다.

powershell -NoProfile -File scripts/preview-quick-msi.ps1 -Mode ready
powershell -NoProfile -File scripts/preview-quick-msi.ps1 -Mode progress
powershell -NoProfile -File scripts/preview-quick-msi.ps1 -Mode complete
powershell -NoProfile -File scripts/preview-quick-msi.ps1 -Mode maintenance
```

디자인 미리보기에는 설치하지 않는다는 안내를 표시하며 버튼은 실제 설치·프로그램 실행을 수행하지 않는다. 시험 MSI의 DemoLaunch.exe는 실행 위치와 전달받은 인수를 메시지로 보여주는 기존 시험 프로그램이다. 결과물은 Git에 포함하지 않는다. 프로젝트는 `examples/QuickMsiDemo.embproj`다.

시험 환경에서 아래 순서로 확인한다.

1. 샘플 MSI를 열어 제품 정보와 설치 후 실행 체크박스가 표시되는지 확인하고 취소한다.
2. 체크한 상태로 신규 설치한다. 성공 시 완료 화면 없이 DemoLaunch 메시지가 나타나고 두 인수 `first argument`, `[literal]`이 각각 전달되는지 확인한다.
3. MSI를 다시 열어 삭제/복구/취소 배치와 실제 복구·제거, 작업별 진행·완료·취소를 확인한다. 유지보수에서 실행 프로그램이 뜨면 실패다.
4. 제품을 제거한 뒤 체크하지 않고 신규 설치한다. 완료 화면에서 실행을 누를 때만 프로그램이 실행되는지 확인한다.
5. 무인 설치·복구·제거에서 Quick UI와 실행 프로그램이 나타나지 않는지, 로그와 MSI 결과를 확인한다.

## 현재 검증과 남은 작업

프로젝트 모드 왕복·구형 XML 기본값, 잘못된 모드 거부, x86/x64 MSI 생성과 DLL PE 아키텍처·콜백 내보내기 이름, Basic UI 비처리 속성, 실행 인수 원문 보존, 성공한 신규 설치만 실행하는 정책을 자동 검사한다. 설치 화면 디자인은 설치 없는 네이티브 미리보기로 확인했다. 이후 화면 자동 확인은 사용자 Esc 입력으로 중단했다.

실제 Embedded UI의 설치·복구·제거, 진행·오류·취소·롤백, INSTALLEND 결과 처리, 실행 체크 여부에 따른 분기와 배율·키보드·보조 기술 호환은 수동 검증이 남아 있다. 사용권 계약, 경로 선택, 사용자 제품 아이콘, 접근성 확대는 후속 단위다. 현재 단계의 구조 검증을 실제 설치 성공으로 간주하지 않는다.

기반 API: [Microsoft Embedded UI](https://learn.microsoft.com/en-us/windows/win32/msi/using-an-embedded-ui), [MsiEmbeddedUI 테이블](https://learn.microsoft.com/en-us/windows/win32/msi/msiembeddedui-table).
