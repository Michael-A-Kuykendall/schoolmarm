# Developer Certificate of Origin

SchoolMarm uses the Developer Certificate of Origin (DCO) as its sign-off mechanism for contributions.

## What is the DCO?

The DCO is a lightweight way for contributors to certify that they wrote or otherwise have the right to submit the code they are contributing to the project.

## The DCO Text

```
Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.


Developer's Certificate of Origin 1.1

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
```

## How to use it

To certify the DCO, you must sign off your commits. This is done by adding a `Signed-off-by` line to your commit messages.

Git can do this automatically for you if you use the `-s` flag:

```
git commit -s -m "Your commit message"
```

The result will look like this:

```
Your commit message

Signed-off-by: Random J Developer <random@developer.example.org>
```
