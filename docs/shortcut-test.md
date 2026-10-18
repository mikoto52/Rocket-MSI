# 시작 메뉴·바탕화면 바로가기

.embproj의 Shortcuts 아래 Shortcut을 여러 개 선언한다. 현재 perMachine 설치이므로 시작 메뉴·바탕화면은 모든 사용자 공용 위치다. 시작 메뉴는 프로그램 목록(ProgramMenuFolder) 아래 Product.name의 폴더를 만든다. 작업 표시줄·시작 화면 고정 기능은 포함하지 않는다.

```xml
<Shortcuts>
  <Shortcut id="menu" location="startMenu" name="내 프로그램"
            target="bin/MyApp.exe" workingDirectory="data" description="프로그램 실행">
    <Arguments><Argument>first argument</Argument><Argument>[literal]</Argument></Arguments>
  </Shortcut>
  <Shortcut id="desktop" location="desktop" name="내 프로그램" target="bin/MyApp.exe" />
</Shortcuts>
```

id는 최대 64자의 영문·숫자·밑줄이며 숫자로 시작할 수 없다. name은 .lnk 확장자 없는 단일 파일명이다. location은 startMenu/desktop이다. target은 입력 폴더에 포함된 파일의 상대 경로이며 EXE 외의 설치 파일도 지정할 수 있다. 외부 경로·상대 상위 경로·폴더 대상은 지원하지 않는다. 같은 위치에서 같은 이름을 중복 선언하면 오류다.

workingDirectory는 입력 폴더 기준이며 생략/`.`는 INSTALLDIR다. 하위 폴더 지정 시 그 폴더 또는 자손에 설치 파일이 있어야 한다. 빈 폴더만 설치하는 기능은 없다. Arguments/Argument는 인수별 문자열이며 빈 문자열·공백·따옴표·대괄호를 보존한다. 현재 바로가기 인수에는 변수 치환을 하지 않는다. description은 최대 255자다. 아이콘은 대상 파일 기본 동작을 사용하며 사용자 아이콘 지정은 후속 범위다.

MSI Shortcut 표준 테이블과 CreateShortcuts/RemoveShortcuts를 사용한다. 바로가기는 대상 파일 컴포넌트에 속하며 광고형 바로가기가 아니다. 삭제된 링크는 MSI의 수동 복구로 복원한다. 자동 복구를 유발하는 광고형 실행은 제공하지 않는다. 제거 시 공용 바탕화면 자체는 지우지 않고, 시작 메뉴 제품 폴더는 비어 있을 때만 제거한다. 같은 이름의 기존 외부 바로가기를 보존하는 충돌 정책은 아직 제공하지 않으므로 제품 고유 이름을 사용한다.

## 시험 키트 생성

```powershell
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\create-shortcut-demo.ps1"
```

dist/shortcut-input에 MessageBox 시험 프로그램을 컴파일하고 examples/ShortcutDemo.embproj로 dist/shortcut-demo/Setup.msi를 생성한다. 기존 폴더는 덮어쓰지 않는다. 시험 프로그램과 MSI를 자동 실행하지 않는다.

## 설치와 실행

```powershell
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\shortcut-demo\Setup.msi" /L*V "D:\sources\Express-MSI-Builder\dist\shortcut-demo\install.log"
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-shortcut-demo.ps1" -Phase Installed
```

설치 완료 후 검사 명령을 실행한다. 기본 설치 경로는 C:\Program Files\InstallEmbly\Shortcut Demo다. 경로를 변경했다면 검사 스크립트에 -InstallDir 인수를 추가한다.

시작 메뉴의 InstallEmbly Shortcut Demo 폴더와 공용 바탕화면에 `InstallEmbly 바로가기 시험`이 생겨야 한다. 각 링크를 직접 실행한다.

| 위치 | 작업 폴더 | 인수 배열 |
|---|---|---|
| 시작 메뉴 | 설치 폴더\data | `"start menu"`, `"[literal]"`, 빈 문자열 |
| 바탕화면 | 설치 폴더 | `"desktop"`, `"인수 하나"`, `a"b` |

메시지 창에서 실행 파일·작업 폴더·인수가 맞는지 확인하고 창을 닫는다. 검사 스크립트는 .lnk 속성만 읽고 프로그램을 실행하지 않는다.

## 복구 및 제거

관리자 PowerShell에서 다음 명령으로 두 링크만 삭제한다. 대상·인수·작업 폴더가 시험 예제와 일치하는지 먼저 검사한다.

```powershell
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-shortcut-demo.ps1" -Phase PrepareRepair
```

같은 MSI에서 복구한 후 다음 명령으로 두 링크의 복원을 확인한다.

```powershell
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-shortcut-demo.ps1" -Phase Repaired
```

복구 중 권한 오류가 나타나면 통과로 간주하지 않고 로그를 확보한다. 관리자 실행 부트스트래퍼는 후속 과제다.

같은 MSI에서 제거한 뒤 다음 명령으로 링크·대상 파일 및 빈 시작 메뉴 폴더가 없어졌는지 확인한다.

```powershell
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-shortcut-demo.ps1" -Phase Removed
```

참고: [Shortcut 테이블](https://learn.microsoft.com/en-us/windows/win32/msi/shortcut-table), [공용 설치 폴더](https://learn.microsoft.com/en-us/windows/win32/msi/installation-context), [빈 폴더 제거](https://learn.microsoft.com/en-us/windows/win32/msi/removefile-table).
