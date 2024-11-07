# 功能
ch3的代码框架为我们实现了一个时钟中断驱动的分时调度系统。我们通过实现下面的关键点来添加一个查询当前任务信息的功能：
* 修改TCB结构，添加了时间字段与系统调用计数数组，并修改其他有关的构建函数
* 为TaskManager实现了`count_syscall`和`get_task_by_id`方法，后者利用了tcb的copy特性
* 修改syscall/mod.rs中实现的syscall函数，在调用时进行一次`count_syscall`计数
* 在process.rs中实现一个sys_taskinfo, 将其注册为系统调用

# 简答题
1. 运行3个bad测例，描述程序出错行为
使用的rustabi-qemu github上的latest版本：`0.1.1`  
输出结果如下：
```bash
# make test CHAPTER=2
[kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003a4, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
[kernel] Panicked at src/syscall/fs.rs:11 called `Result::unwrap()` on an `Err` value: Utf8Error { valid_up_to: 3, error_len: Some(1) }
```
显然，内核是不允许我们在用户态访问非法地址和执行S态指令的。访问0x0会pagefault. traphandler会kill掉这个任务。

2. 刚进入`__restore`时，`a0`代表了什么值。请指出`__restore`的两种使用情景。
* `__restore`一是用于创建初始任务（初始化taskmanager）的时候，二是在从内核态切换回用户态的时候用于切换（恢复）上下文
* 如果是从内核trap恢复，那么a0存储了traphandler的返回值
* 如果是创建初始任务，a0可能存储了上下文相关的context的信息


3. 这几行汇编代码特殊处理了哪些寄存器？这些寄存器的的值对于进入用户态有何意义？请分别解释。
```asm
ld t0, 32*8(sp)
ld t1, 33*8(sp)
ld t2, 2*8(sp)
csrw sstatus, t0
csrw sepc, t1
csrw sscratch, t2
```
处理了t0-2和三个CSR：sstatus, sepc, sscratch, 前三个用于寄存器间临时传送，后三个寄存器对于进入用户态意义比较大。sstatus保存当前特权级信息，sepc保存了代码的入口点，sscratch保存了用户态的堆栈。


4. 为何跳过了 x2 和 x4？
```asm
ld x1, 1*8(sp)
ld x3, 3*8(sp)
.set n, 5
.rept 27
   LOAD_GP %n
   .set n, n+1
.endr
```
x2在riscv中用于堆栈指针sp，x4则用于线程指针tp. 这两个寄存器在切换上下文时要单独处理


5. 该指令之后，sp 和 sscratch 中的值分别有什么意义？
```asm
csrrw sp, sscratch, sp
```
两者发生了交换，现在sp指向用户态栈而sscratch指向内核栈


6. `__restore：`中发生状态切换在哪一条指令？为何该指令执行之后会进入用户态？
发生于`sret`。`sret`指令会根据 sstatus 寄存器中的 SPP 位决定返回到用户模式还是特权模式。


7. 从U态进入 S 态是哪一条指令发生的？
一般来说，是在发生trap的时候调用ecall指令由硬件更新的。在核心库中应该有实现。


# 荣誉准则
在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

无

此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

无，均为自己完成

1. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

2. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。