<template>
  <div>
    <el-row :gutter="20">
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
            <div class="stat-value">{{ stats.avg_qps_5m ? stats.avg_qps_5m.toFixed(1) : 0 }}</div>
            <div class="stat-label">5分钟平均QPS</div>
          </div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card shadow="hover">
          <div class="stat-card">
            <div class="stat-value">{{ stats.avg_qps_1h ? stats.avg_qps_1h.toFixed(1) : 0 }}</div>
            <div class="stat-label">1小时平均QPS</div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-card shadow="hover" style="margin-top: 20px">
      <template #header>
        <div style="display: flex; justify-content: space-between; align-items: center">
          <span>QPS趋势</span>
          <div>
            <el-select v-model="timeRange" style="width: 120px; margin-right: 12px" @change="loadHistory">
              <el-option label="5分钟" :value="5" />
              <el-option label="15分钟" :value="15" />
              <el-option label="30分钟" :value="30" />
              <el-option label="1小时" :value="60" />
              <el-option label="6小时" :value="360" />
            </el-select>
            <el-button type="primary" @click="loadData" :icon="'Refresh'" />
          </div>
        </div>
      </template>
      <div ref="chartRef" style="height: 400px"></div>
    </el-card>

    <el-card shadow="hover" style="margin-top: 20px">
      <template #header><span>API接口QPS分布</span></template>
      <el-table :data="apiStats" stripe border>
        <el-table-column prop="api_path" label="API路径" />
        <el-table-column prop="count" label="请求数" sortable />
        <el-table-column prop="qps" label="QPS" sortable>
          <template #default="{ row }">{{ row.qps.toFixed(2) }}</template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, nextTick, watch } from 'vue'
import { getQpsStats, getQpsHistory } from '../api'
import * as echarts from 'echarts'

const stats = ref({})
const apiStats = ref([])
const timeRange = ref(5)
const chartRef = ref(null)
let chartInstance = null
let timer = null

async function loadData() {
  try {
    const res = await getQpsStats()
    stats.value = res.data || {}
    apiStats.value = res.data?.api_stats || []
  } catch {}
}

async function loadHistory() {
  try {
    const res = await getQpsHistory({ minutes: timeRange.value })
    const data = res.data || []
    if (!chartInstance && chartRef.value) {
      chartInstance = echarts.init(chartRef.value)
    }
    if (!chartInstance) return

    chartInstance.setOption({
      tooltip: { trigger: 'axis' },
      legend: { data: ['QPS'] },
      grid: { left: '3%', right: '4%', bottom: '3%', containLabel: true },
      xAxis: {
        type: 'time',
        axisLine: { lineStyle: { color: '#ddd' } }
      },
      yAxis: {
        type: 'value',
        name: 'QPS',
        splitLine: { lineStyle: { type: 'dashed', color: '#eee' } }
      },
      series: [{
        name: 'QPS',
        type: 'line',
        smooth: true,
        data: data.map(d => [new Date(d.recorded_at).getTime(), d.total_qps]),
        areaStyle: {
          color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
            { offset: 0, color: 'rgba(64,158,255,0.3)' },
            { offset: 1, color: 'rgba(64,158,255,0.05)' }
          ])
        },
        lineStyle: { color: '#409EFF', width: 2 },
        itemStyle: { color: '#409EFF' }
      }]
    })
  } catch {}
}

watch(chartRef, () => {
  if (chartRef.value && !chartInstance) {
    chartInstance = echarts.init(chartRef.value)
    loadHistory()
  }
})

onMounted(() => {
  loadData()
  loadHistory()
  timer = setInterval(loadData, 10000)
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
