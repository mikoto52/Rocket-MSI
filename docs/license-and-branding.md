# 이용약관 및 브랜딩

## 현재 구현

`--license <파일>` 지정 시 환영 → 사용권 계약 → 설치 폴더 선택 → 설치 준비 완료 → 진행 → 완료 순서다. 미지정 시 기존 흐름을 유지한다. 약관을 MSI에 포함하므로 설치 대상 PC에 원본 파일이 필요하지 않다. 페이지 제목은 사용권 계약이며 안내는 “다음의 사용권 계약을 잘 읽어보시기 바랍니다.”이다. 약관 본문은 테두리가 있는 스크롤 박스로 표시한다.

- TXT: UTF-8(BOM 선택) 지원. 줄바꿈, 탭, 한글과 보조 평면 문자를 RTF Unicode escape로 변환한다. 중괄호와 역슬래시는 이스케이프한다.
- RTF: 초기 구현은 ASCII 기반 RTF를 지원한다. 한글 등 비ASCII 문자는 RTF의 Unicode/hex escape로 저장해야 한다. 원시 ANSI 바이트, UTF-16 파일 및 바이너리 데이터가 들어간 문서는 초기 지원 범위가 아니다.
- 최대 원본 크기: 1 MiB. 빈 파일, NUL 포함, 잘못된 확장자 및 기본 RTF 헤더 오류는 빌드 전에 거부한다. 완전한 RTF 문법 검증기는 아니며 복잡한 서식의 표시 품질은 실제 화면에서 확인한다.
- MSI ScrollableText로 표시하며 제품 변수는 치환하지 않는다. 예를 들어 `[ProductName]`은 원문이다.

동의 체크박스 기본값은 해제다. `ACCEPTLICENSE="1"`일 때만 다음 버튼을 활성화하고 화면 이동 이벤트에도 같은 조건을 적용한다. 이전 화면으로 돌아갔을 때에는 같은 세션의 체크 상태를 유지한다. 취소하면 설치를 진행하지 않는다. 끝까지 스크롤해야 동의 가능한 정책은 현재 적용하지 않는다.

무인 설치도 신규 설치 시 명시적인 `ACCEPTLICENSE=1`이 필요하다. 실행 시퀀스의 LaunchConditions가 확인하며 자동으로 동의하지 않는다. 설치된 제품의 유지보수에는 재동의를 요구하지 않는다. 체크 상태는 법적 동의 이력/서명 기록을 저장하는 기능과는 다르다.

## BrandingText

기본값은 **Rocket MSI**다. GUI의 브랜딩 문구, 프로젝트의 `<UI><BrandingText>...</BrandingText></UI>`, 직접 소스 빌드 CLI의 `--branding-text "문구"`로 변경한다. 하단뿐 아니라 환영·진행·완료·중단 화면의 마법사 이름에도 같은 값을 사용한다. 제품명은 별도 설정이다.

하단 문구는 배너 페이지에만 표시한다. 환영·완료·중단 페이지는 왼쪽 이미지 구성을 사용한다. 하단 X=2/Y=234/Height=12이며 Width는 빌드 시 실제 폰트의 글자 폭과 여유 폭을 반영해 1~386 범위에서 계산한다. 구분선은 Y=240에서 전체 폭을 유지하며 불투명 문구 배경과 컨트롤 순서로 겹침을 처리한다. 매우 긴 문구는 잘릴 수 있고, 실행 시 BrandingText 속성을 변경해도 너비를 재계산하지 않는다.

한 줄 문구만 허용한다. 빈 문구는 표시할 텍스트가 없다. 이전 기본값 Express MSI Builder와 InstallEmbly는 프로젝트를 읽을 때 Rocket MSI로 변환하고, 다른 사용자 지정 문구는 보존한다. 화면별 동작과 검증 범위는 [설치 UI 안내](installer-ui.md)를 따른다.

## 초기 시험 이력

아래 결과물과 날짜별 기록은 초기 구현 당시의 시험 이력이다. 현재 기본값과 화면 구성은 위 설명을 따른다. `dist`의 결과물은 로컬 시험 파일이며 Git에 포함하지 않는다.

## 시험 결과물

- `dist/emb-demo-terms.msi`: 한글 TXT 예시 + 기본 브랜딩
- `dist/emb-demo-terms-rtf.msi`: RTF 예시 + 브랜딩

샘플 약관은 UI 시험용이며 실제 제품 약관이나 프로젝트 소스 라이선스가 아니다. 현재 `.embproj` 로더가 없으므로 CLI로 전달한다. 향후 프로젝트 XML에 약관 파일과 BrandingText를 저장할 계획이다.

2026-10-18: 변환 단위 테스트, TXT/RTF MSI 생성, 동의 속성 기본값 부재, 버튼 상태 조건 및 실행 시퀀스 조건, 브랜딩 기본값·변경값·위치 대상, 기존 1620 회귀 검사 통과. Windows Installer 세션 열기는 프로세스 내 순차 검사로 수행한다. 정적 검사 통과. 실제 UI 클릭·RTF 렌더링 및 무인 설치의 실행 결과는 아직 미검증이다.

## 수동 시험

브랜딩 겹침 수정(2026-10-18): 불투명 배경만으로는 선이 문구를 관통한다는 사용자 보고를 받았다. 모든 컨트롤을 Control_First/Control_Next 순환 체인에 명시하고 선 → BrandingText 순서로 배치했다. 선은 전체 너비로 유지한다. 모든 배너 화면의 체인 완결성과 그리기 순서는 자동 검사하며 실제 겹침 표시는 재확인이 필요하다. 수정본은 `dist/emb-demo-branding-order.msi`다. 참고: [InstallShield 컨트롤 그리기 순서](https://community.revenera.com/s/article/making-a-control-over-a-bitmap-visible-on-msi-dialog).

깨끗한 시험 VM 또는 이전 시험 제품을 제거한 환경에서 확인한다. 기존 시험 제품과 ProductCode가 같으므로 기존 제품이 있으면 유지보수 화면이 나올 수 있다.

```powershell
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\emb-demo-terms.msi" /L*v "D:\sources\Express-MSI-Builder\dist\install-terms.log"
```

확인할 사항: 한글·스크롤·RTF 서식, 동의 전 다음/Enter 차단, 동의 후 진행, 체크 해제 시 재차단, 이전/다음 이동, 취소, 환영 화면의 브랜딩 제외 및 나머지 화면 표시. 별도 무인 시험에서는 동의 속성 미지정 시 실패, `ACCEPTLICENSE=1` 지정 시 진행되는지 확인한다.

공식 참고: [ScrollableText](https://learn.microsoft.com/en-us/windows/win32/msi/scrollabletext-control), [ControlCondition](https://learn.microsoft.com/en-us/windows/win32/msi/controlcondition-table).
