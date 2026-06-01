<template>
  <div>
    <el-row :gutter="20">
      <el-col :span="6">
        <el-card shadow="hover">
          <div class="stat-card">
            <div class="stat-value">{{ stats.total_requests || 0 }}</div>
            <div class="stat-label">总请求数</div>
          </div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card shadow="hover">
          <div class="stat-card">
            <div class="stat-value">{{ stats.current_qps || 0 }}</div>
            <div class="stat-label">当前QPS</div>
          </div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card shadow="hover">
          <div class="stat-card">
            <div class="stat-value">{{ stats.avg_qps_1m ? stats.avg_qps_1m.toFixed(1) : 0 }}</div>
            <div class="stat-label">1分钟平均QPS</div>
          </div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card shadow="hover">
          <div class="stat-card">
            <div class="stat-value">{{ devCount }}</div>
            <div class="stat-label">开发者总数</div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-card shadow="hover" style="margin-top: 20px">
      <template #header>
        <span>API QPS 分布</span>
      </template>
      <div ref="chartRef" style="height: 350px"></div>
    </el-card>

    <el-row :gutter="20" style="margin-top: 20px">
      <el-col :span="12">
        <el-card shadow="hover">
          <template #header><span>系统状态</span></template>
          <el-descriptions :column="1" border>
            <el-descriptions-item label="PostgreSQL">
              <el-tag :type="health.pg ? 'success' : 'danger'" size="small">
                {{ health.pg ? '正常' : '异常' }}
              </el-tag>
            </el-descriptions-item>
            <el-descriptions-item label="Redis">
              <el-tag :type="health.redis ? 'success' : 'danger'" size="small">
                {{ health.redis ? '正常' : '异常' }}
              </el-tag>
            </el-descriptions-item>
            <el-descriptions-item label="服务状态">
              <el-tag :type="health.ok ? 'success' : 'danger'" size="small">
                {{ health.ok ? '运行中' : '已暂停' }}
              </el-tag>
            </el-descriptions-item>
          </el-descriptions>
        </el-card>
      </el-col>
      <el-col :span="12">
        <el-card shadow="hover">
          <template #header><span>最近交易</span></template>
          <el-table :data="recentTxns" size="small" max-height="250" v-loading="loadingTxns">
            <el-table-column prop="transaction_token" label="Token" min-width="120" show-overflow-tooltip />
            <el-table-column prop="amount" label="数量" width="80" />
            <el-table-column prop="status" label="状态" width="80">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)" size="small">{{ row.status }}</el-tag>
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, nextTick } from 'vue'
import { getQpsStats, getTransactions, getDevelopers } from '../api'
import axios from 'axios'
import * as echarts from 'echarts'

const stats = ref({})
const devCount = ref(0)
const health = ref({ pg: true, redis: true, ok: true })
const recentTxns = ref([])
const loadingTxns = ref(false)
const chartRef = ref(null)
let chartInstance = null
let timer = null

function statusType(s) {
  const map = { pending: 'warning', committed: 'success', cancelled: 'info', expired: 'danger' }
  return map[s] || 'info'
}

async function loadData() {
  try {
    const s = await getQpsStats()
    stats.value = s.data || {}
  } catch {}
  try {
    const d = await getDevelopers({ page: 1, page_size: 1 })
    devCount.value = d.data?.total || 0
  } catch {}
  try {
    const t = await getTransactions({ page: 1, page_size: 10 })
    recentTxns.value = t.data?.data || []
  } catch {}
  try {
    const h = await axios.get('/health')
    health.value = { pg: true, redis: true, ok: h.data.status === 'healthy' }
  } catch {
    health.value = { pg: false, redis: false, ok: false }
  }
}

function initChart() {
  if (!chartRef.value) return
  chartInstance = echarts.init(chartRef.value)
  updateChart()
}

function updateChart() {
  if (!chartInstance || !stats.value.api_stats) return
  const data = stats.value.api_stats || []
  chartInstance.setOption({
    tooltip: { trigger: 'axis' },
    xAxis: { type: 'category', data: data.map(d => d.api_path) },
    yAxis: { type: 'value', name: 'QPS' },
    series: [{
      type: 'bar',
      data: data.map(d => d.qps),
      itemStyle: { color: '#409EFF', borderRadius: [4, 4, 0, 0] }
    }]
  })
}

onMounted(() => {
  loadData()
  nextTick(initChart)
  timer = setInterval(loadData, 15000)
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
  if (chartInstance) chartInstance.dispose()
})
</script>

<style scoped>
.stat-card { text-align: center; padding: 10px 0; }
.stat-value { font-size: 36px; font-weight: 700; color: #303133; }
.stat-label { font-size: 14px; color: #909399; margin-top: 8px; }
</style>
