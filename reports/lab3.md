# 荣誉准则

1. 在完成本次实验的过程 (含此前学习的过程) 中，我曾分别与 **以下各位** 就 (与本次实验相关的) 以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

> 我没有与其他人交流过

2. 此外，我也参考了 **以下资料** ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

> 无参考资料

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人 (含此后各届同学) 复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按 “-100” 分计

# 总结

## 迁移之前的 syscall

`sys_get_time` 改动不大。mmap 和 munmap 中的辅助函数，随着 `change_program_brk` 函数的改动，挪到了 `TaskControlBlock` 的 trait 里。

## 进程创建 (sys\_spawn)

根据 spawn 等于 fork 加 exec 的思想，参考 TaskControlBlock 中 exec 和 fork 函数的实现，实现了 spawn 函数。主要注意的点：

1. 相比 exec，spawn 需要创建一个新的子进程，因此要像 fork 一样，为子进程分配相应的内容 (但页表需要创建新的)。
2. 相比 fork，spawn 的 TrapContext 需要进行大更新。

## stride 调度算法

首先为 TaskControlBlock 添加了 stride 和 priority 的字段以及对应的 get/set 方法。
同时添加了 `sys_set_priority` 系统调用，并修改了 TaskManager 的 `fetch` 函数，使其能够根据文档中的 stride 调度算法选择进程。

# 问答题

## 第 1 问

实际情况是轮到 p2 执行，因为 p2.stride (250) 加上 10 后会溢出，变为 4。

## 第 2 问

考虑极端情况，在进程优先级全部都为 2 的情况下，那么所有进程的 pass 均为 BigStride / 2，很容易推算出 `STRIDE_MAX – STRIDE_MIN == BigStride / 2`。如果存在优先级大于 2 的进程，那么就会出现更小的 pass，从而
`STRIDE_MAX – STRIDE_MIN < BigStride / 2`。

## 第 3 问

看不懂 guide 在说什么，根据自己的猜测来写了。

```rust
fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    // 将 u8 转换为 i8 的语义：>=128 视为负数
    let a = if self.0 < 128 {
        self.0 as i16 // 正数
    } else {
        (self.0 as i16) - 256 // 负数(例如 128u8 → -128i16)
    };
    let b = if other.0 < 128 {
        other.0 as i16
    } else {
        (other.0 as i16) - 256
    };

    // 直接比较转换后的值
    a.partial_cmp(&b)
}
```