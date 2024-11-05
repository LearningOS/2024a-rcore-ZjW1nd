# 实验内容
* 基于修改过的task模块重整了先前实现的gettime, taskinfo, mmap等调用函数
* 将sys_fork, sys_execve, tcb.fork(), tcb.exec()缝合实现了sys_spawn函数
* 简单实现了prio的修改

# 简答题
1. 实际情况是轮到p1执行吗？为什么？
显然并不是。p2在被调度后`stide+=10`，发生溢出变为4, 由变得比p1小，因此调度又会接着执行p2.
2. 为什么STRIDE_MAX – STRIDE_MIN <= BigStride / 2？
这个公式反应的是当前所有进程里优先级最高和最低的进程的stride相差不会太大，从而能防止溢出。

在优先级全部大于等于2的情况下，pass步长也会小于等于BigStride/2。即使优先级最高，pass最小为1,优先级即使最低pass也为BigStide/2，调度一次后二者相差不会超过bigstride/2.

3. 简单的比较器：
```rust
use core::cmp::Ordering;

struct Stride(u8);

impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let BigStride = 255;
        let half_BigStride = BigStride / 2;
        // 计算差值，考虑溢出情况
        let diff = self.0.wrapping_sub(other.0);
        if diff <= half_BigStride {
            Some(Ordering::Less)
        } else {
            Some(Ordering::Greater)
        }
    }
}

impl PartialEq for Stride {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}
```
# 荣誉准则
在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

无

此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

Copilot

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。