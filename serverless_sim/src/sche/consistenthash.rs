use crate::{
    fn_dag::EnvFnExt,
    mechanism::{MechanismImpl, ScheCmd, SimEnvObserve, UpCmd},
    mechanism_thread::{MechCmdDistributor, MechScheduleOnceRes},
    node::EnvNodeExt,
    request::Request,
    sim_run::{schedule_helper, Scheduler},
    with_env_sub::WithEnvCore,
};

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

pub struct ConsistentHashScheduler {
    upper_limit: f32,
}

impl ConsistentHashScheduler {
    pub fn new() -> Self {
        Self { upper_limit: 0.8 }
    }

    fn schedule_one_req_fns(
        &mut self,
        env: &SimEnvObserve,
        mech: &MechanismImpl,
        req: &mut Request,
        cmd_distributor: &MechCmdDistributor,
    ) {
        let fns = schedule_helper::collect_task_to_sche(
            req,
            env,
            schedule_helper::CollectTaskConfig::All,
        );

        // let mut sche_cmds = vec![];
        // let mut scale_up_cmds = vec![];

        for fnid in fns {
            let mut target_cnt = mech.scale_num(fnid);
            if target_cnt == 0 {
                target_cnt = 1;
            }

            let mut hasher = DefaultHasher::new();
            fnid.hash(&mut hasher);
            let mut node_id = hasher.finish() as usize % env.node_cnt();
            let mut node = env.node(node_id);
            let mut node_mem_use_rate = node.unready_mem() / node.rsc_limit.mem;
            let _nodes_left_mem = env
                .core()
                .nodes()
                .iter()
                .map(|n| n.left_mem_for_place_container())
                .collect::<Vec<_>>();
            while node_mem_use_rate > self.upper_limit {
                node_id = (node_id + 1) % env.node_cnt();
                node = env.node(node_id);
                node_mem_use_rate = node.unready_mem() / node.rsc_limit.mem;
            }
            cmd_distributor
                .send(MechScheduleOnceRes::ScheCmd(ScheCmd {
                    nid: node_id,
                    reqid: req.req_id,
                    fnid,
                    memlimit: None,
                }))
                .unwrap();
            // sche_cmds.push();
            while target_cnt != 0 {
                if node.container(fnid).is_none() {
                    cmd_distributor
                        .send(MechScheduleOnceRes::ScaleUpCmd(UpCmd {
                            nid: node_id,
                            fnid,
                        }))
                        .unwrap();
                }
                node_id = (node_id + 1) % env.node_cnt();
                node = env.node(node_id);
                target_cnt -= 1;
            }
        }
    }
}

impl Scheduler for ConsistentHashScheduler {
    fn schedule_some(
        &mut self,
        env: &SimEnvObserve,
        mech: &MechanismImpl,
        cmd_distributor: &MechCmdDistributor,
    ) {
        // let mut up_cmds = vec![];
        // let mut sche_cmds = vec![];
        // let mut down_cmds = vec![];

        for func in env.core().fns().iter() {
            let target = mech.scale_num(func.fn_id);
            let cur = env.fn_container_cnt(func.fn_id);
            if target < cur {
                // down_cmds.extend(
                mech.scale_down_exec().exec_scale_down(
                    env,
                    func.fn_id,
                    cur - target,
                    cmd_distributor,
                );
            }
        }

        for (_req_id, req) in env.core().requests_mut().iter_mut() {
            // let (sub_up, sub_down, sub_sche) =
            self.schedule_one_req_fns(env, mech, req, cmd_distributor);
        }
    }
}
