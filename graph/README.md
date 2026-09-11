# 图谱组件依赖

来源是用户指定目录对应的独立仓库 `https://github.com/zjarlin/az-compose.git`。
`source.lock.json` 锁定提交和文件集合；`scripts/prepare-graph.mjs` 生成忽略提交的 `src/`。
只引入通用绘制、模型、布局与遍历，不引入原应用主题、编辑器或业务服务。
记忆类型通过前端适配器转成图谱节点和颜色，不修改组件的业务枚举。
