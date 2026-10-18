# 분할 CAB 단위

최신 사용자 검증(2026-10-18): 분할 CAB 시험본에 대해 **통과** 보고를 받았다. 아래 구현 당시의 정상 설치·복구·제거 시험 대기 상태를 갱신한다. CAB 누락·혼합·손상 및 실제 4GiB 이상 대용량 경계 시험까지 통과한 것으로 간주하지 않는다.

2026-10-18 구현. CLI의 `--cab-size-mib N`으로 CAB당 크기를 지정한다. 정수 1–2047 MiB를 허용하며, 옵션 생략 시 기존 단일 CAB 방식이다. `--cab-storage embedded|external` 양쪽에서 사용할 수 있다.

```powershell
cargo run --offline -- build dist/split-input dist/split-external/Setup.msi --cab-storage external --cab-size-mib 1 --license examples/demo-terms.txt
```

## 분할 정책

원본 파일 순서와 크기 합으로 묶음을 나누고, 각 묶음을 별도 CAB로 만든다. 실제 CAB 크기가 한도를 넘으면 묶음을 다시 나눈다. 파일 하나의 원본 크기가 한도를 넘거나, CAB 헤더와 압축 결과를 포함한 크기가 한도를 넘으면 빌드를 실패시킨다. 압축이 잘 되는 파일이라도 원본이 한도를 넘으면 거부하는 보수적인 정책이다. 단일 파일을 CAB 경계에서 자르는 spanning은 아직 지원하지 않는다.

내장 분할은 #Data1.cab, #Data2.cab 스트림을 사용하고 외부 분할은 Data1.cab, Data2.cab 파일을 사용한다. Media.DiskId 및 LastSequence는 실제 파일 범위에 맞춰 작성한다. 외부 CAB는 8.3 이름 제약에 따라 최대 9999개다. 내장 CAB를 나눠도 최종 MSI 자체의 크기 한도가 해결되는 것은 아니다. 실제 4GiB 이상 지원은 아직 검증하지 않았다.

모든 CAB를 작업 폴더에서 생성·검증하고 외부 CAB 전체를 먼저 공개한 뒤 MSI를 마지막에 공개한다. 중간 CAB 이름 충돌이나 MSI 공개 실패 시 이번에 만든 파일만 정리하고 기존 파일은 보존한다. 강제 종료·전원 장애의 원자성은 보장하지 않는다.

## 시험본

- 외부: `dist/split-external/Setup.msi` + `Data1.cab` + `Data2.cab` + `Data3.cab`
- 내장: `dist/split-embedded/Setup.msi`

입력은 scripts/create-split-demo.ps1로 만든 700KiB 파일 세 개이며, 각 CAB 한도는 1MiB다. 생성된 각 외부 CAB는 717257바이트로 한도 안에 있다. 예상 설치 파일은 data/file1.bin, data/file2.bin, data/file3.bin이다.

시험 환경에 이전 시제품이 있으면 먼저 제거한다. 외부 시험본은 MSI와 CAB 세 개를 같은 폴더에 함께 둔다.

```powershell
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\split-external\Setup.msi" /L*v "D:\sources\Express-MSI-Builder\dist\split-external\install.log"
```

설치 후 세 파일의 크기·내용 확인 → 서로 다른 CAB에 속한 파일 삭제 후 복구 → 제거 순서로 시험한다. 내장 방식도 동일하게 확인한다. CAB 하나 누락·다른 빌드 CAB 혼합·손상 상황은 별도 실패 시험이며 정상 설치 성공만으로 완료 처리하지 않는다.

## 자동 검증

6개 테스트와 Clippy 검사 대상. 내장/외부 3개 CAB 생성, Media 파일 범위, 내장 스트림 존재, 외부 CAB별 크기 및 실제 해제 후 원본 대조, 단일 파일 한도 초과, 실제 압축 결과가 큰 경우 추가 분할, 두 번째 CAB 충돌 시 첫 CAB 정리를 검사한다. 기존 약관·브랜딩·유지보수·1620 회귀 검사도 유지한다. 실제 설치·복구·제거 및 대용량 경계 시험은 미완료다.
