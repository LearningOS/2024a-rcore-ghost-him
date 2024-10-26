# 实现的功能

完成了编程作业：支持获取当前任务的信息。所有的任务信息都存放在了TaskControlBlock中。

由于TaskManager可以访问TaskControlBlock，所以向TaskManager中添加对应的接口，从而实现对这些信息的修改与获取。

而对于其他的模块，则是通过调用TaskManager的接口来实现对这些信息的修改与获取。

对于获取当前任务的信息，则都是直接调用TaskManager的查询接口来实现的。而对于修改任务使用的系统调用及调用次数，由于每个系统调用都会通过syscall函数来实现对系统调用的分发，同时，由于每个时刻都只有一个任务在运行，所以通过修改syscall函数来完成对该信息的记录。

# 简答作业

## 程序的出错行为

当前的sbi为

```bash
[rustsbi] RustSBI version 0.3.0-alpha.2, adapting to RISC-V SBI v1.0.0
```

bad_address的出错信息为

```bash
[kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003c8, kernel killed it.
```

bad_instructions的出错信息为：

```bash
[kernel] IllegalInstruction in application, kernel killed it.
```

bad_register的出错信息为

```bash
[kernel] IllegalInstruction in application, kernel killed it.
```

## trap.S中两个函数的作用

0. __alltraps的作用为处理从用户态向内核态的转换，保存Trap的上下文。而__restore则是从栈上trap的上下文恢复陷入内核态之前的状态。

1. a0存放的是内核栈的栈顶。有两种方式可以使用__restore，一种是直接调用该函数一种是在处理完trap后自动执行该函数。

2. L43-L48处理了sstatus，sepc,sscratch这三个寄存器
* sstatus用于记录当前cpu处理于个特权级（用户态与内核态）
* sepc用于记录在进入Trap之前所执行的最后一条指令的地址
* sscratch用于记录内核栈顶所在的位置，在这里是将sscratch恢复成内核栈顶。

3. 对于x2，x2为堆栈寄存器，其存放的是用户栈顶，他在上文已经读过了，将其值存放在了sscratch寄存器中。所以在读通用寄存器时不需要再读了。而x4寄存器在程序中用不到，所以不需要读。

4. 在没有执行该指令时，sp指向的是内核栈，而sscratch指向的是用户栈，执行完以后，会将这两个寄存器存放的内容互换
5. 在执行了46行的sret这一行后，会进入用户态。该指令在执行后恢复所有必要的寄存器和状态。
6. sp指向用户栈顶，而sscratch指向内核栈顶
7. csrrw sp, sscratch, sp这一行从用户态进入到了内核态。这个实现了从用户栈到内核栈的切换。

# 荣誉准则

在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

无

此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

* https://learningos.cn/rCore-Camp-Guide-2024A
* https://rcore-os.cn/rCore-Tutorial-Book-v3

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

# 改进点

一开始看上面两章的时候，还是比较懵的。如果可以多加一些说明，或者多来一些拓展链接，比如指向rCore-Tutorial-Book-v3的，这样会更加容易理解。（因为我对risc-v是一点了解都没有，看到不会的就只能百度或问ai）

还有一些东西，比如，系统调用的id，一开始并不知道为什么要设计成这样，不知道为什么SYSCALL_WRITE的值要设置为64，而不是其他的值，如果可以多来一些说明会比较好；还有在一开始写操作系统时，不知道为什么要把操作系统的开始地址设置为那个固定的值。

作业的整体难度不高，有基本的代码功底就可以写出来了。