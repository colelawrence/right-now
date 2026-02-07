# Task ID Parsing Test Fixture

## Tasks with 3-letter prefix IDs
- [ ] First task [abc.first-task]
- [ ] Second task [xyz.second-task-123]

## Tasks with 4-letter prefix IDs
- [ ] Long prefix task [abcd.long-prefix-task]
- [x] Completed task [wxyz.completed-task]

## Tasks with ID + session badge
- [ ] Running task [def.running-task] [Running](todos://session/42)
- [ ] Stopped task [ghi.stopped-task] [Stopped](todos://session/99)
- [ ] Waiting task [jkl.waiting-task] [Waiting](todos://session/123)

## Tasks with badge but no ID
- [ ] Badge only running [Running](todos://session/1)
- [ ] Badge only stopped [Stopped](todos://session/2)

## Tasks with special characters in label
- [ ] Hyphenated name [mno.multi-word-label]
- [ ] Numbers included [pqr.label-with-123]
- [ ] Complex label [stu.some-complex-label-456]

## Tasks without ID or badge
- [ ] Plain task one
- [x] Plain completed task
