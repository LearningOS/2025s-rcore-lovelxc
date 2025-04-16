# 荣誉准则

1. 在完成本次实验的过程 (含此前学习的过程) 中，我曾分别与 **以下各位** 就 (与本次实验相关的) 以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

> 我没有与其他人交流过

2. 此外，我也参考了 **以下资料** ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

> 无参考资料

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人 (含此后各届同学) 复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按 “-100” 分计

# 总结

本章代码实现了对每个 Task 使用过的 syscall 进行追踪。虽然实验指导建议对 `TaskManagerInner` 进行修改，但我认为在 `TaskControlBlock` 里添加 trace info 更加合适。

为了使其他模块能够修改与获取 trace info，在 task 模块中添加了 `trace_syscall` 和 `get_syscall_times` 函数。

在 `syscall` 函数中，使用 `trace_syscall` 记录系统调用信息；在 `sys_trace` 系统调用里，实现了实验所要求的功能，特别的，当 `trace_request=2` 时，使用 `get_syscall_times` 函数返回信息。

# 问答题

## 第 1 题

SBI 版本如下：

```
[rustsbi] RustSBI version 0.3.0-alpha.2, adapting to RISC-V SBI v1.0.0
[rustsbi] Implementation     : RustSBI-QEMU Version 0.2.0-alpha.2
```

使用 `make run CHAPTER=2` 来跑 bad 测例，结果如下：

```C
// ch2b_bad_address.rs
PageFault in application, bad addr = 0x0, bad instruction = 0x804003a4, kernel killed it.
// ch2b_bad_register.rs
IllegalInstruction in application, kernel killed it.
// ch2b_bad_instructions.rs
IllegalInstruction in application, kernel killed it.
```

ch2b\_bad\_address.rs 尝试往地址为 0 的内存写入数据，导致了 PageFault。ch2b\_bad\_register.rs 和 ch2b\_bad\_instructions.rs 都尝试在 U-mode 下执行 S-mode 指令，导致了 IllegalInstruction。

## 第 2 题

### 1

刚进入 `__restore` 时，`sp` 代表的是指向内核栈上分配的 TrapContext 结构体的指针，包含了所有需要恢复的上下文信息

`__restore` 的两种使用情景：

1. 在准备进入用户态应用前，使用`__restore` 将内核态构造好的上下文恢复给用户栈
2. 在内核态处理完 Trap 后，使用`__restore` 恢复用户态应用的上下文

### 2

分别处理了 sstatus (Supervisor Status Register)，sepc (Supervisor Exception Program Counter) 和 sscratch (Supervisor Scratch Register) 三个寄存器

1. sstatus: 确保处理器在返回用户态时有正确的特权级别 (SPP) 和中断状态 (SPIE)
2. sepc: 当执行 sret 指令时，处理器会将 PC 设置为 sepc 的值，确保用户程序正确恢复执行
3. sscratch: 保存用户栈指针，稍后通过 `csrrw sp, sscratch, sp` 指令与内核栈交换

### 3

阅读`__alltraps` 的代码与注释，可以知道：
x2 (sp) 栈指针寄存器的正确恢复方式是：

```asm
addi sp, sp, 34*8       # 释放 TrapContext 空间
csrrw sp, sscratch, sp  # 与 sscratch 中存储的用户栈指针交换
```

在当前情况下，操作系统还没有线程的概念，因此 x4 (tp) 线程指针寄存器没必要恢复，而且`__alltraps` 也没有保存 x4 (tp) 寄存器，也恢复不出来。

### 4

sp 指向用户栈 (为返回用户态做准备)，而 sscratch 存储内核栈指针 (为下一次陷入内核态做准备)。

### 5

`sret` 指令发生了状态切换。在前面的代码中，已通过 `csrw sstatus, t0` 恢复了正确的 sstatus 值，sret 指令会读取 sstatus 寄存器中的 SPP 位 (为 0)，从而切换到用户态

### 6

与第 4 题相反，sp 指向内核栈 (为返回内核态做准备)，而 sscratch 存储用户栈指针 (保存用户栈指针，以便稍后恢复)。

### 7

`ecall` 指令触发系统调用并进入 Trap。在 Trap 返回时，U-mode 执行的下一条指令是 ecall 指令的 pc + 4 的指令。

# 我的看法

建议制作 devcontainer.json 配置文件。

由于此课程主要面向国内同学，建议在 Dockerfile 中将 Ubuntu 以及 rust 的源换成国内的。

第三章实验有点过于简单了，不知道是不是我有一定基础的缘故。
